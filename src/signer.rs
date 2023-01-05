use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{EncryptedPrivateKey, Event, KeySecurity, PreEvent, PrivateKey, PublicKey};
use tokio::task;

pub struct Signer {
    public: Option<PublicKey>,
    encrypted: Option<EncryptedPrivateKey>,
    private: Option<PrivateKey>,
}

impl Default for Signer {
    fn default() -> Signer {
        Signer {
            public: None,
            encrypted: None,
            private: None,
        }
    }
}

impl Signer {
    pub async fn load_from_settings(&mut self) {
        let settings = GLOBALS.settings.read().await;
        *self = Signer {
            public: settings.public_key,
            encrypted: settings.encrypted_private_key.clone(),
            private: None
        };
    }

    pub async fn save_through_settings(&self) -> Result<(), Error> {
        let mut settings = GLOBALS.settings.write().await;
        settings.public_key = self.public.clone();
        settings.encrypted_private_key = self.encrypted.clone();
        settings.save().await
    }

    #[allow(dead_code)]
    pub fn set_public_key(&mut self, pk: PublicKey) {
        if self.private.is_some() {
            *GLOBALS.status_message.blocking_write() = "Ignored setting of public key (private key supercedes)".to_string();
        } else {
            self.public = Some(pk);
        }
    }

    #[allow(dead_code)]
    pub fn set_encrypted_private_key(&mut self, epk: EncryptedPrivateKey) {
        if self.private.is_some() && self.encrypted.is_some() {
            // ignore, epk supercedes
        } else {
            self.encrypted = Some(epk);
        }
    }

    pub fn set_private_key(&mut self, pk: PrivateKey, pass: &str) -> Result<(), Error> {
        self.encrypted = Some(pk.export_encrypted(&pass)?);
        self.public = Some(pk.public_key());
        self.private = Some(pk);
        Ok(())
    }

    pub fn unlock_encrypted_private_key(&mut self, pass: &str) -> Result<(), Error> {
        if self.private.is_some() {
            // ignore, already unlocked
            Ok(())
        }
        else if let Some(epk) = &self.encrypted {
            self.private = Some(epk.decrypt(pass)?);
            Ok(())
        } else {
            Err(Error::NoPrivateKey)
        }
    }

    pub fn generate_private_key(&mut self, pass: &str) -> Result<(), Error> {
        let pk = PrivateKey::generate();
        self.encrypted = Some(pk.export_encrypted(pass)?);
        self.public = Some(pk.public_key());
        self.private = Some(pk);
        Ok(())
    }

    pub fn is_loaded(&self) -> bool {
        self.encrypted.is_some() || self.private.is_some()
    }

    pub fn is_ready(&self) -> bool {
        self.private.is_some()
    }

    pub fn public_key(&self) -> Option<PublicKey> {
        self.public.clone()
    }

    pub fn encrypted_private_key(&self) -> Option<EncryptedPrivateKey> {
        self.encrypted.clone()
    }

    pub fn key_security(&self) -> Option<KeySecurity> {
        if let Some(pk) = &self.private {
            Some(pk.key_security())
        } else {
            None
        }
    }

    pub fn sign_preevent(&self, preevent: PreEvent, pow: Option<u8>) -> Result<Event, Error> {
        match &self.private {
            Some(pk) => match pow {
                Some(pow) => Ok(Event::new_with_pow(preevent, &pk, pow)?),
                None => Ok(Event::new(preevent, &pk)?),
            },
            _ => Err(Error::NoPrivateKey),
        }
    }

    pub fn export_private_key_bech32(&mut self, pass: &str) -> Result<String, Error> {
        match &self.encrypted {
            Some(epk) => {
                // Test password
                let mut pk = epk.decrypt(pass)?;

                let output = pk.try_as_bech32_string()?;

                // We have to regenerate encrypted private key because it may have fallen from
                // medium to weak security. And then we need to save that
                let epk = pk.export_encrypted(pass)?;
                self.encrypted = Some(epk);
                self.private = Some(pk);
                task::spawn(async move {
                    if let Err(e) = GLOBALS.signer.read().await.save_through_settings().await {
                        tracing::error!("{}", e);
                    }
                });
                Ok(output)
            }
            _ => Err(Error::NoPrivateKey),
        }
    }

    pub fn export_private_key_hex(&mut self, pass: &str) -> Result<String, Error> {
        match &self.encrypted {
            Some(epk) => {
                // Test password
                let mut pk = epk.decrypt(pass)?;

                let output = pk.as_hex_string();

                // We have to regenerate encrypted private key because it may have fallen from
                // medium to weak security. And then we need to save that
                let epk = pk.export_encrypted(pass)?;
                self.encrypted = Some(epk);
                self.private = Some(pk);
                task::spawn(async move {
                    if let Err(e) = GLOBALS.signer.read().await.save_through_settings().await {
                        tracing::error!("{}", e);
                    }
                });
                Ok(output)
            }
            _ => Err(Error::NoPrivateKey),
        }
    }

    pub fn delete_identity(&mut self, pass: &str) -> Result<(), Error> {
        match &self.encrypted {
            Some(epk) => {
                // Verify their password
                let _pk = epk.decrypt(pass)?;

                self.private = None;
                self.encrypted = None;
                self.public = None;
                task::spawn(async move {
                    if let Err(e) = GLOBALS.signer.read().await.save_through_settings().await {
                        tracing::error!("{}", e);
                    }
                });
                Ok(())
            }
            _ => Err(Error::NoPrivateKey),
        }
    }
}
