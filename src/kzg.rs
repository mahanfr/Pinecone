use ark_bls12_381::{Bls12_381, Fr, G1Projective, G2Projective};
use ark_ec::pairing::Pairing;
use ark_ff::{One, PrimeField, UniformRand, Zero};
use ark_poly::{DenseUVPolynomial, Polynomial, univariate::{DenseOrSparsePolynomial, DensePolynomial}};
use ark_std::test_rng;

pub struct KZG {
    pub tau: Fr,
    pub g1: G1Projective,
    pub g2: G2Projective,
    pub powers_g1: Vec<G1Projective>,
    pub powers_g2: Vec<G2Projective>,
}
impl KZG {
    pub fn new(max_degree: usize) -> Self {
        // TODO: This is unsafe please consider using KZG trusted setup
        let mut rng = test_rng();
        let tau = Fr::rand(&mut rng);
        let g1 = G1Projective::rand(&mut rng);
        let g2 = G2Projective::rand(&mut rng);

        let mut powers_g1 = Vec::with_capacity(max_degree);
        let mut powers_g2 = Vec::with_capacity(max_degree);
        let mut tau_pow = Fr::one();

        for _ in 0..=max_degree {
            powers_g1.push(g1 * tau_pow);
            powers_g2.push(g2 * tau_pow);
            tau_pow *= tau;
        }

        Self { tau, g1, g2, powers_g1, powers_g2 }
    }

    // C = f(s) * G1
    pub fn commit(&self, poly: &DensePolynomial<Fr>) -> G1Projective {
        let mut commitment = G1Projective::zero();
        for (i, coeff) in poly.coeffs().iter().enumerate() {
            if i < self.powers_g1.len() {
                commitment += self.powers_g1[i] * coeff;
            }
        }
        commitment
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

        let (quotient, remainder) = numerator_sparse.divide_with_q_and_r(&divisor_sparse).expect("Division failed");
        assert_eq!(remainder, DensePolynomial::zero());

        let proof = self.commit(&quotient);
        (y, proof)
    }

    // e(C-yG1, G2) == e(π, tau * G2 - z * G2)
    pub fn verify(&self, commitment: G1Projective, index: u8, y: Fr, proof: G1Projective) -> bool {
        let index_fr = Fr::from(index);
        let lhs = Bls12_381::pairing(commitment - self.g1 * y, self.g2);
        let rhs = Bls12_381::pairing(proof, self.g2 * self.tau - self.g2 * index_fr);
        lhs == rhs
    }

    // TODO: Real Verkle uses Pedersen hashing to map child commitments to field
    pub fn hash_to_scalar(data: &[u8]) -> Fr {
        let hash = blake3::hash(data).as_bytes().to_owned();
        Fr::from_le_bytes_mod_order(&hash)
    }
}
