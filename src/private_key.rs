use core::convert::TryFrom;

use num_bigint::{BigInt, Sign};
use rand_core::{CryptoRng, RngCore};
use sha3::{
    digest::{ExtendableOutput, Update},
    Shake256,
};

use crate::{
    array_to_key, point::Point, public_key::PublicKey, shake256, Ed448Error, PreHash, KEY_LENGTH,
    SIG_LENGTH,
};

pub type PrivateKeyRaw = [u8; KEY_LENGTH];
pub type SeedRaw = [u8; KEY_LENGTH];

pub struct PrivateKey(PrivateKeyRaw);

opaque_debug::implement!(PrivateKey);

impl PrivateKey {
    /// Generate a random key.
    ///
    /// # Example
    ///
    /// ```
    /// use rand_core::{RngCore, OsRng};
    /// use ed448_rust::PrivateKey;
    /// let private_key = PrivateKey::new(&mut OsRng);
    /// ```
    pub fn new<T>(rnd: &mut T) -> Self
    where
        T: CryptoRng + RngCore,
    {
        let mut key = [0; KEY_LENGTH];
        rnd.fill_bytes(&mut key);
        Self::from(key)
    }

    /// Convert the private key to a format exportable.
    ///
    /// # Example
    ///
    /// ```
    /// # use rand_core::{RngCore, OsRng};
    /// # use ed448_rust::PrivateKey;
    /// # let private_key = PrivateKey::new(&mut OsRng);
    /// let exportable_pkey = private_key.as_bytes();
    /// ```
    pub fn as_bytes(&self) -> &[u8; KEY_LENGTH] {
        &self.0
    }

    pub(crate) fn expand(&self) -> (PrivateKeyRaw, SeedRaw) {
        // 1.  Hash the 57-byte private key using SHAKE256(x, 114), storing the
        //     digest in a 114-octet large buffer, denoted h.
        let h = Shake256::default()
            .chain(self.as_bytes())
            .finalize_boxed(114);
        //     Only the lower 57 bytes are used for generating the public key.
        let mut s = array_to_key(&h[..KEY_LENGTH]);

        // 2.  Prune the buffer: The two least significant bits of the first
        //     octet are cleared, all eight bits the last octet are cleared, and
        //     the highest bit of the second to last octet is set.
        s[0] &= 0b1111_1100;
        s[56] = 0;
        s[55] |= 0b1000_0000;

        let seed = array_to_key(&h[KEY_LENGTH..]);

        (s, seed)
    }

    /// Sign with key pair.
    ///
    /// It's possible to indicate a context. More information in
    /// [RFC8032#8.3 Use of contexts](https://tools.ietf.org/html/rfc8032#section-8.3).
    ///
    /// # Examples
    ///
    /// Without any context.
    ///
    /// ```
    /// use ed448_rust::PrivateKey;
    /// let pkey = PrivateKey::from([0xcd, 0x23, 0xd2, 0x4f, 0x71, 0x42, 0x74, 0xe7, 0x44, 0x34, 0x32, 0x37, 0xb9,
    ///             0x32, 0x90, 0xf5, 0x11, 0xf6, 0x42, 0x5f, 0x98, 0xe6, 0x44, 0x59, 0xff, 0x20, 0x3e, 0x89, 0x85,
    ///             0x08, 0x3f, 0xfd, 0xf6, 0x05, 0x00, 0x55, 0x3a, 0xbc, 0x0e, 0x05, 0xcd, 0x02, 0x18, 0x4b, 0xdb,
    ///             0x89, 0xc4, 0xcc, 0xd6, 0x7e, 0x18, 0x79, 0x51, 0x26, 0x7e, 0xb3, 0x28]);
    /// let msg = &[0x0c, 0x3e, 0x54, 0x40, 0x74, 0xec, 0x63, 0xb0, 0x26, 0x5e, 0x0c];
    /// let sig = pkey.sign(msg, None).unwrap();
    ///
    /// assert_eq!(
    ///     sig.iter().map(|b| format!("{:02x}", b)).collect::<Vec<String>>().concat(),
    ///     "1f0a8888ce25e8d458a21130879b840a9089d999aaba039eaf3e3afa090a09d389dba82c4ff2ae8a\
    ///         c5cdfb7c55e94d5d961a29fe0109941e00b8dbdeea6d3b051068df7254c0cdc129cbe62db2dc9\
    ///         57dbb47b51fd3f213fb8698f064774250a5028961c9bf8ffd973fe5d5c206492b140e00"
    /// );
    /// ```
    ///
    /// With a context.
    ///
    /// ```
    /// use ed448_rust::PrivateKey;
    /// let pkey = PrivateKey::from([0xc4, 0xea, 0xb0, 0x5d, 0x35, 0x70, 0x07, 0xc6, 0x32, 0xf3, 0xdb, 0xb4, 0x84,
    ///             0x89, 0x92, 0x4d, 0x55, 0x2b, 0x08, 0xfe, 0x0c, 0x35, 0x3a, 0x0d, 0x4a, 0x1f, 0x00, 0xac, 0xda,
    ///             0x2c, 0x46, 0x3a, 0xfb, 0xea, 0x67, 0xc5, 0xe8, 0xd2, 0x87, 0x7c, 0x5e, 0x3b, 0xc3, 0x97, 0xa6,
    ///             0x59, 0x94, 0x9e, 0xf8, 0x02, 0x1e, 0x95, 0x4e, 0x0a, 0x12, 0x27, 0x4e]);
    /// let msg = &[03];
    /// let sig = pkey.sign(msg, Some(&[0x66, 0x6f, 0x6f])).unwrap();
    ///
    /// assert_eq!(
    ///     sig.iter().map(|b| format!("{:02x}", b)).collect::<Vec<String>>().concat(),
    ///     "d4f8f6131770dd46f40867d6fd5d5055de43541f8c5e35abbcd001b32a89f7d2151f7647f11d8ca2\
    ///         ae279fb842d607217fce6e042f6815ea000c85741de5c8da1144a6a1aba7f96de42505d7a7298\
    ///         524fda538fccbbb754f578c1cad10d54d0d5428407e85dcbc98a49155c13764e66c3c00"
    /// );
    /// ```
    ///
    /// # Error
    ///
    /// It could return `[Ed448Error::ContextTooLong]` if the context is more than
    /// 255 byte length.
    pub fn sign(&self, msg: &[u8], ctx: Option<&[u8]>) -> crate::Result<[u8; SIG_LENGTH]> {
        self.sign_real(msg, ctx, PreHash::False)
    }

    /// Sign with key pair. Message is pre-hashed before signed.
    ///
    /// The message is hashed before being signed. The size of the signed message in this
    /// case is always 64 bytes length.
    ///
    /// See `[PrivateKey::sign]`
    pub fn sign_ph(&self, msg: &[u8], ctx: Option<&[u8]>) -> crate::Result<[u8; SIG_LENGTH]> {
        self.sign_real(msg, ctx, PreHash::True)
    }

    fn sign_real(
        &self,
        msg: &[u8],
        ctx: Option<&[u8]>,
        pre_hash: PreHash,
    ) -> crate::Result<[u8; SIG_LENGTH]> {
        let ctx = ctx.unwrap_or(b"");
        if ctx.len() > 255 {
            return Err(Ed448Error::ContextTooLong);
        }

        let msg = match pre_hash {
            PreHash::False => Box::from(msg),
            PreHash::True => Shake256::default().chain(msg).finalize_boxed(64),
        };
        // Expand key.
        let (a, seed) = &self.expand();
        let a = BigInt::from_bytes_le(Sign::Plus, a);
        // Calculate r and R (R only used in encoded form).
        let r = shake256(vec![seed, &msg], ctx, pre_hash);
        let r = BigInt::from_bytes_le(Sign::Plus, r.as_ref()) % Point::l();
        let R = (Point::default() * &r).encode();
        // Calculate h.
        let h = shake256(
            vec![&R, PublicKey::from(a.clone()).as_byte(), &msg],
            ctx,
            pre_hash,
        );
        let h = BigInt::from_bytes_le(Sign::Plus, h.as_ref()) % Point::l();
        // Calculate s.
        let S = (r + h * a) % Point::l();
        // The final signature is a concatenation of R and S.
        let mut S_ = S.magnitude().to_bytes_le();
        S_.resize_with(KEY_LENGTH, Default::default);
        let S = array_to_key(&S_);

        let mut result = [0; SIG_LENGTH];
        result.copy_from_slice(&[R, S].concat());
        Ok(result)
    }
}

impl From<PrivateKeyRaw> for PrivateKey {
    /// Restore the private key from the slice.
    fn from(array: PrivateKeyRaw) -> Self {
        Self(array)
    }
}

impl TryFrom<&'_ [u8]> for PrivateKey {
    type Error = Ed448Error;

    /// Restore the private key from an array.
    ///
    /// # Error
    ///
    /// Could return `[Ed448Error::WrongKeyLength]` if the array's length
    /// is not `[KEY_LENGTH]`.
    fn try_from(bytes: &[u8]) -> crate::Result<Self> {
        if bytes.len() != KEY_LENGTH {
            return Err(Ed448Error::WrongKeyLength);
        }
        Ok(PrivateKey::from(array_to_key(bytes)))
    }
}

impl From<&'_ PrivateKeyRaw> for PrivateKey {
    fn from(bytes: &PrivateKeyRaw) -> Self {
        PrivateKey::from(*bytes)
    }
}