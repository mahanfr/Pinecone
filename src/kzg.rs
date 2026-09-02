use ark_bls12_381::{Bls12_381, Fr, G1Affine, G1Projective, G2Projective};
use ark_ec::{CurveGroup, VariableBaseMSM, pairing::Pairing};
use ark_ff::{Field, One, PrimeField, UniformRand, Zero};
use ark_poly::{
    DenseUVPolynomial, EvaluationDomain ,GeneralEvaluationDomain, Polynomial, univariate::{DenseOrSparsePolynomial, DensePolynomial}
};
use ark_serialize::CanonicalSerialize;
use ark_std::test_rng;

pub struct KZG {
    pub tau: Fr,
    pub g1: G1Projective,
    pub g2: G2Projective,
    pub powers_g1: Vec<G1Affine>,
    pub domain: GeneralEvaluationDomain<Fr>,
    pub powers_g1_lagrange: Vec<G1Affine>,
}
impl KZG {
    pub fn new(max_degree: usize) -> Self {
        // TODO: This is unsafe please consider using KZG trusted setup
        let mut rng = test_rng();
        let tau = Fr::rand(&mut rng);
        let g1 = G1Projective::rand(&mut rng);
        let g2 = G2Projective::rand(&mut rng);

        let mut powers_g1 = Vec::with_capacity(max_degree + 1);
        let mut tau_pow = Fr::one();

        for _ in 0..=max_degree {
            powers_g1.push((g1 * tau_pow).into_affine());
            tau_pow *= tau;
        }

        // TODO: in case of real tau use group element FFT
        let domain = GeneralEvaluationDomain::<Fr>::new(max_degree + 1)
            .expect("domain size must support FFT");
        let n = domain.size();
        let n_inv = Fr::from(n as u64).inverse().unwrap();
        let vanishing_at_tau = domain.evaluate_vanishing_polynomial(tau);
        let powers_g1_lagrange: Vec<G1Affine> = domain.elements()
            .map(|omega_i| {
                let denom = (tau - omega_i).inverse()
                    .expect("tau collided with domain point");
                let l_i = omega_i * n_inv * vanishing_at_tau * denom;
                (g1 * l_i).into_affine()
            }).collect();

        Self {
            tau,
            g1,
            g2,
            powers_g1,
            domain,
            powers_g1_lagrange
        }
    }

    // C = f(s) * G1
    pub fn commit(&self, poly: &DensePolynomial<Fr>) -> G1Projective {
        let n = poly.coeffs().len();
        assert!(
            n <= self.powers_g1.len(),
            "polynomial exceeds KZG setup degree"
        );
        let bases = &self.powers_g1[..n];
        let scalars = poly.coeffs();
        G1Projective::msm_unchecked(bases, scalars)
    }

    pub fn commit_sparse_lagrange(&self, entries: &[(usize, Fr)]) -> G1Projective {
        if entries.is_empty() {
            return G1Projective::zero();
        }
        if entries.len() == 1 {
            let (index, scalar) = entries[0];
            if scalar.is_zero() {
                return G1Projective::zero();
            }
            return self.powers_g1_lagrange[index] * scalar;
        }
        let mut bases = Vec::with_capacity(entries.len());
        let mut scalars = Vec::with_capacity(entries.len());
        for &(index, scalar) in entries {
            if scalar.is_zero() {
                continue;
            }
            assert!(
                index < self.powers_g1_lagrange.len(),
                "coefficients index exceeds KZG setup"
            );
            bases.push(self.powers_g1_lagrange[index]);
            scalars.push(scalar);
        }
        if scalars.is_empty() {
            return G1Projective::zero();
        }
        G1Projective::msm_unchecked(&bases, &scalars)
    }

    pub fn open(&self, poly: &DensePolynomial<Fr>, z: Fr) -> (Fr, G1Projective) {
        let y = poly.evaluate(&z);
        let numerator = poly - &DensePolynomial::from_coefficients_vec(vec![y]);
        let divisor = DensePolynomial::from_coefficients_vec(vec![-z, Fr::one()]);

        let numerator_sparse = DenseOrSparsePolynomial::from(&numerator);
        let divisor_sparse = DenseOrSparsePolynomial::from(&divisor);

        let (quotient, remainder) = numerator_sparse
            .divide_with_q_and_r(&divisor_sparse)
            .expect("KZG polynomial division failed");

        assert!(remainder.is_zero(), "f(X) - f(z) is not divisible by X-z");
        let proof = self.commit(&quotient);
        (y, proof)
    }

    // q(X) = f(X) - y / X - z where y = f(z)
    pub fn prove(&self, poly: &DensePolynomial<Fr>, index: u8) -> (Fr, G1Projective) {
        // z
        let index_fr = Fr::from(index);
        // y = f(z)
        let y = poly.evaluate(&index_fr);
        // f(X) - y
        let numerator = poly - &DensePolynomial::from_coefficients_vec(vec![y]);
        // X - index
        let divisor = DensePolynomial::from_coefficients_vec(vec![-index_fr, Fr::one()]);

        let numerator_sparse = DenseOrSparsePolynomial::from(&numerator);
        let divisor_sparse = DenseOrSparsePolynomial::from(&divisor);

        let (quotient, remainder) = numerator_sparse
            .divide_with_q_and_r(&divisor_sparse)
            .expect("Division failed");
        assert_eq!(remainder, DensePolynomial::zero());

        let proof = self.commit(&quotient);
        (y, proof)
    }

    // e(C-yG1, G2) == e(π, tau * G2 - z * G2)
    pub fn verify(&self, commitment: G1Projective, z: Fr, y: Fr, proof: G1Projective) -> bool {
        let lhs = Bls12_381::pairing(commitment - self.g1 * y, self.g2);
        let rhs = Bls12_381::pairing(proof, self.g2 * (self.tau - z));
        lhs == rhs
    }

    pub fn verify_batch(&self, items: &[(G1Projective, Fr, Fr, G1Projective)]) -> bool {
        if items.is_empty() { return true;}

        let mut transcript = Vec::new();
        for (c, z, y, pi) in items {
            c.serialize_compressed(&mut transcript).unwrap();
            z.serialize_compressed(&mut transcript).unwrap();
            y.serialize_compressed(&mut transcript).unwrap();
            pi.serialize_compressed(&mut transcript).unwrap();
        }
        let challenges: Vec<Fr> = (0..items.len())
            .map(|i| {
                let mut seed = transcript.clone();
                seed.extend_from_slice(&(i as u64).to_le_bytes());
                KZG::hash_to_scalar(&seed)
            }).collect();
        let mut a = G1Projective::zero();
        let mut b = G1Projective::zero();
        let mut d = G1Projective::zero();

        for ((c, z, y, pi), r) in items.iter().zip(challenges.iter()) {
            a += (*c - self.g1 * y) * r;
            b += *pi * r;
            d += *pi * (*z * r);
        }

        let lhs = Bls12_381::pairing(a + d, self.g2);
        let rhs = Bls12_381::pairing(b, self.g2 * self.tau);
        lhs == rhs
    }

    // TODO: Real Verkle uses Pedersen hashing to map child commitments to field
    pub fn hash_to_scalar(data: &[u8]) -> Fr {
        let digset = blake3::hash(data);
        Fr::from_le_bytes_mod_order(digset.as_bytes())
    }

    pub fn hash_g1_to_scalar(point: &G1Projective) -> Fr {
        let mut bytes = Vec::new();
        point
            .serialize_compressed(&mut bytes)
            .expect("G1 serialization failed");
        Self::hash_to_scalar(&bytes)
    }
}
