use encoding_rs::{Encoding, SHIFT_JIS, WINDOWS_1252};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LanguageError {
    #[error("Error encoding string to {0} bytes")]
    EncodeError(String),

    #[error("Error decoding as {0} bytes")]
    DecodeError(String),

    #[error("The number {0} does not correspond to any supported language")]
    InvalidLanguageValue(u8),
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Language {
    English = 1,
    French = 3,
    German = 2,
    Japanese = 0,
    Spanish = 4,
}

impl Language {
    pub fn from_number(number: u8) -> Result<Language, LanguageError> {
        use Language::*;
        let language = match number {
            0 => Japanese,
            1 => English,
            2 => German,
            3 => French,
            4 => Spanish,
            n => return Err(LanguageError::InvalidLanguageValue(n)),
        };
        Ok(language)
    }

    pub fn get_encoding(&self) -> &'static Encoding {
        use Language::*;
        match self {
            // we should technically be using ISO-8859-1, but encoding_rs does not have it ???
            // this is probably close enough at any rate ...
            English | French | German | Spanish => WINDOWS_1252,
            Japanese => SHIFT_JIS,
        }
    }

    pub fn decode_text(&self, bytes: &[u8]) -> Result<String, LanguageError> {
        let encoding = self.get_encoding();
        let (cow, encoding_used, had_errors) = encoding.decode(bytes);
        if had_errors {
            Err(LanguageError::DecodeError(encoding_used.name().to_string()))
        } else {
            Ok(cow.to_string())
        }
    }

    pub fn encode_text(&self, s: &str) -> Result<Vec<u8>, LanguageError> {
        let encoding = self.get_encoding();
        let (cow, encoding_used, had_errors) = encoding.encode(s);
        if had_errors {
            Err(LanguageError::EncodeError(encoding_used.name().to_string()))
        } else {
            Ok(cow.to_vec())
        }
    }
}

#[cfg(test)]
mod tests {
    use claim::*;

    use super::*;

    #[test]
    pub fn language_encode_decode() {
        assert_eq!(
            "The East Tower",
            Language::English
                .decode_text(&[
                    0x54, 0x68, 0x65, 0x20, 0x45, 0x61, 0x73, 0x74, 0x20, 0x54, 0x6f, 0x77, 0x65,
                    0x72
                ])
                .unwrap()
        );

        assert_eq!(
            vec![
                0x54, 0x68, 0x65, 0x20, 0x45, 0x61, 0x73, 0x74, 0x20, 0x54, 0x6f, 0x77, 0x65, 0x72
            ],
            Language::English.encode_text("The East Tower").unwrap()
        );

        assert_eq!(
            "東天の塔",
            Language::Japanese
                .decode_text(&[0x93, 0x8c, 0x93, 0x56, 0x82, 0xcc, 0x93, 0x83])
                .unwrap()
        );

        assert_eq!(
            vec![0x93, 0x8c, 0x93, 0x56, 0x82, 0xcc, 0x93, 0x83],
            Language::Japanese.encode_text("東天の塔").unwrap()
        );

        assert_matches!(
            Language::English.encode_text("東天の塔"),
            Err(LanguageError::EncodeError(_))
        );
    }
}
