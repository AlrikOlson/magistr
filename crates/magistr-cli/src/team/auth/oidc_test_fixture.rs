//! Locally generated RSA keys for deterministic offline OIDC tests only.

use base64::prelude::*;
use chrono::{DateTime, Utc};
use jsonwebtoken::jwk::{Jwk, JwkSet, KeyOperations, PublicKeyUse};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde_json::{Value, json};

pub(crate) struct OidcTestFixture {
    kid: String,
    key: EncodingKey,
}

impl OidcTestFixture {
    pub(crate) fn new(kid: &str) -> Self {
        Self::with_key(kid, KEY_ONE)
    }

    pub(crate) fn other(kid: &str) -> Self {
        Self::with_key(kid, KEY_TWO)
    }

    fn with_key(kid: &str, encoded: &str) -> Self {
        let der = BASE64_STANDARD.decode(encoded).expect("fixture DER");
        Self {
            kid: kid.into(),
            key: EncodingKey::from_rsa_der(&der),
        }
    }

    pub(crate) fn jwks(&self) -> JwkSet {
        let mut key = Jwk::from_encoding_key(&self.key, Algorithm::RS256).expect("fixture JWK");
        key.common.key_id = Some(self.kid.clone());
        key.common.public_key_use = Some(PublicKeyUse::Signature);
        key.common.key_operations = Some(vec![KeyOperations::Verify]);
        JwkSet { keys: vec![key] }
    }

    pub(crate) fn sign(&self, claims: &Value) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.kid.clone());
        self.sign_header(&header, claims)
    }

    pub(crate) fn sign_header(&self, header: &Header, claims: &Value) -> String {
        jsonwebtoken::encode(header, claims, &self.key).expect("sign fixture")
    }

    pub(crate) fn claims(issuer: &str, client_id: &str, now: DateTime<Utc>) -> Value {
        json!({
            "iss": issuer,
            "aud": client_id,
            "sub": "user-1",
            "iat": now.timestamp(),
            "exp": now.timestamp() + 3600,
            "email": "alice@example.dev",
            "name": "Alice Ng"
        })
    }
}

// These keys were generated for this fixture and never used as credentials.
const KEY_ONE: &str = "MIIEowIBAAKCAQEAuoveBVw7yhKVsqWjC+oSeTDokrFF2UFYCBhtkoGi7oICeVUGYGrWlD2fWiyk6tTLYwR9/p5YfeEPe7HiCUz9gkSCehNMwvdRftmU9ONNemSPE0l5k/8LpcZ5m8bDGqM5LjBF5FxyU0pJXmE5Cu+wO0RFEl5L0m2vqUlxaZq3nKxb15p/lf5iqBjQY3xwpxhTULF1AWOJ1ZQDmyWHLxdkojFcxPsrEtdEpIg3aNTrSp/mxaqvtgsMqROXe0ZAliyY9MXFJoBpWzEM4WqC3k7pTC1XdOjvnhQqpnsgEj67TivMTveKTxT0j1codszagBaJgkswg16deJzKHjBda1wz+wIDAQABAoIBACTp9YK5CCnJ+K5zwWpjpMkviO4yPyCpIgrkDpatqXip6GRrE0p0Cccm1IgrCm1fs2mCI46Fdfxe00oGXrS5c/bZqLoQZKG1//3qf2PgMN411Owbn3/OOddRiZHuSIBWaUO/EK2KL6NUsWUINxn3aTepIEPsvfNDej21B0i+8XOCfzbUBR+GptSiP2hQQQA4UYb0AGYXhEwyYQgTKtALGFhWeuzjlrPWeSZvQSZGWvMd+h1KiN9feuRTeGWmWJO83YcqcACXEqpl0sGYb/M8dfTZBlGS+DH4NQGaixR3QDwuHO6tmnN5QrrXtGSvOff0GHnThbTDKzOpuvGIfVGEzBUCgYEA6NK/obRJzb+GBw2/5KDhceCkWtc/IPYre4ocExxawhCJzdLiRy6Ws29xBixksGO7i5Km7UJpK2ti2A72acPvAqIGJ2fMJCbWO9XRKik1KajPHj9E/+kPRg1Kxikyg9bzGlWIb55J9N7bqAh/FZR2wMoGhI/7L3MEfYdRNllcV9cCgYEAzR3NRs85I7kHGDSW8hdXrxC8P+BV4/fNvBUtrcT7ddHDmprV7gocPt+CVIz1VVqwalWs6Nui3JOF9OMLRG09xP0CBMm1M/PD5nH6OGi80/RcutTKINJf58U6Z7pvSdhbFxZkX3k9craQTZop/LjkBvE39VlMoXA4/d7OW0/7MH0CgYBj/IYBuvxrGNJFWzlKxXCsSC4tOP1KeY3I+1H9PxMW/qU/uE4nBbIAk151qUEAo4Qad7uqbvfdhrunUB2w5E1T0yNTbvczm14ryjKOqa3EG1P/iGpogL1ndzPikDS6e+hhKdRzIwZd3At3SVbSfLTXLKvJ6E+WXA3UnvVYot3OdwKBgBNZ927xx/tFzpSLCZ6ncx9SHeCM4q2SSHexK9EUH4E7HBAvAXXmwElyPtKJRfAakUBG+vwxFIlo5Z93+C0pfUVjdVhMS2suWVIZeLjvsLjyefkqcDNuhZvRMLRuk2dNITLpaujTAGgdLzoTDFkKfIDnKxdbqG4mr0nMRn246wLlAoGBAOJ07/vm8M6IZQiGQRuPzwUjhPsALbGHyNYRpD6Z9ZNSPnWh7j5BOoFjpTeAKogUOlXD+LNNNMyJYOPF3QcK+RD067BMHUbNOR6CjpstDrabPIT6r+FABxXSQ7T+qV+mJIWc/HesPMLMHNGJ166hy+LMZJY0Sn2t67At0SLgSEcB";
const KEY_TWO: &str = "MIIEowIBAAKCAQEAuasF5dmWtnciRIiXKgOeOyzRAAlOHaMld4GnMZbAbKXiKBdBIUXiBxf0ipcEm66zvoh/SqPACRPP/CaAYZfaFIp+gEZPRcRLYutOT+Gqg+J5qhKeiQocYw9iA59rxBilVZOuBVTGs3s09AfXegGA+QlwkQIEF0JR+xmeADnLG34eUZiSRkw473nbnQu3PR1oytr2IDplhxMnHa40ZLii/5J/s7BUdd73qGK1o49cRR6+x4kYkKGIFUq7OsQv4LuS+iLoxuD3KID6XdM8Q9J8iLbiZzmy632QIowSGSnx74ip00fopVeW3GikBDn/ReS/xDmSG70d6hmn6hHGZ34TfQIDAQABAoIBACROgLM/Nt0Mc9uB+EljdrwsmdwPdcKO5iM168tjJtK4J37eeUJ352iqswus115YLdbq2zVtyv4bbTbqEn3DE4ajxHPyC8thRLqdt+0f/FeY4GC1DmJO41ktp2V89bZdeWtAGy34s3hideroY6b8h0troKMCiWlA80YObtqtcn23+Oubah+OuhAFnGOavcIM6SM+ZkQFCY70hHBs6cq2nP80GpTRG4Azsw0MFXjELKseJprVflMh3k8T+D0t7MKHXpgqM1lDIyqZa0K/R/orXWBnI7jzPrxxo64BOIiAVmXkkUsYfkBkcHAG6f75KHqh3R/UOK1pdUj1V3TvHiGTIwMCgYEA53noCopJ6koRAEFAPOuAzGXc+CnUlpZt5mp9DELCNij+rRCqsUvsiZhrM3uJZLdwckBfQWTAdUpQ9NhHXensejhH+0b8O1CkWBtsAfrzk6rVkict3NdhVIsT0eIppuAhDYrfposlNUl+zec3ZGLmyf9kp205MTpz1OwVkgXZDF8CgYEAzVa1hQId+zZSFhK7D3jruKrMAJxj/723No2rn8Z3HXfM2Rm+lkiEFqc9rt19mnr1B2HL9gWDBLshGdnFFXkolfwjgnMwbGy1u9XIr2dggHUyUqfNHlPJIYU2vUn4g782aAXaJLNB9KgNANqT2Z/58JTQHMdDPSua6Lr0cVVxraMCgYEAtggmNHO/4OFSMVMfA0loRlUsynOK3Ml7r6G6qPOu6wyeCtd6hGbiouocE6Zoy61+XjrFLhXfS92fgIy8wtv8vbiY0sBmBvdJ5ofaDkydVFkaDw+Ef5FY1zVv5rPmcXb8RzwY2U2/ewJLF/1mkCk+uUeAjjHUsiP3h1AJPoYexB0CgYBOR3f7tMrpMdQpRgq540c5pyMOOJCGtjRaP7fluSs/uKQHP6FAeXShjjAdsaJoeedfYRD4QTGpZA1CZZqUjRPD2dNPr8oOC4NckCtpcud/ES6WBBJWNWq/tKEhmEl9MC/1iAWZaB0Ev+v1H16t4CbQaGrODci8URtFDcCSVBh6MQKBgAhFHpNOzw1VxStlhakiePtnJL/QDQy98Fpul9iL+U35o2bmRsftEz0VSvuuT1X3Dt4zqD0csUVAKX2/uYlta5ZJCDPg6xzscExh0PM4QIIgniXB3AQKIG8r8njcfFCY6Fb1K+k+iL93ztveecbj8PQ1t+5wvBqhXeltr9p5JaW3";
