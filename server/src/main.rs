use hyper::Uri;
use jsonwebtoken::EncodingKey;
use octocrab::{models::AppId, Octocrab};
use orion::hazardous::mac::hmac::sha256::SecretKey;
use server::config::{load_github_app_config, GitHubAppConfiguration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_tracing()?;
    setup_crypto()?;
    let app_config = load_github_app_config()?; //.unwrap_or(create_dummy_config());

    tokio::try_join!(
        server::public_app::<Octocrab>(app_config),
        server::internal_app()
    )?;
    Ok(())
}

fn setup_tracing() -> Result<(), Box<dyn std::error::Error>> {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .try_init()?;

    Ok(())
}

fn setup_crypto() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[allow(dead_code)]
fn create_dummy_config() -> GitHubAppConfiguration {
    let secret = SecretKey::from_slice(&[0; 32]).unwrap();
    GitHubAppConfiguration {
        webhook_secret: secret,
        app_identifier: AppId(1),
        app_key: {
            use rand::SeedableRng;
            use rsa::pkcs8::EncodePrivateKey;
            // let mut rng = rand::thread_rng();
            let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(17_832_551);
            let bits = 2048;
            let priv_key =
                rsa::RsaPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
            let _pub_key = rsa::RsaPublicKey::from(&priv_key);

            let der_encoded_key = priv_key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).unwrap();
            let cert_pem_str = der_encoded_key.to_string();
            println!("{cert_pem_str}");

            EncodingKey::from_rsa_pem(cert_pem_str.as_bytes()).unwrap()
        },
        uri: Uri::from_static("https://github.local"),
    }
}
