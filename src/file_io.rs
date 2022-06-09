use log::info;
use std::path::PathBuf;

pub struct FileInputOutput {
    pub unencoded_filename: PathBuf,
    pub encoded_filename: PathBuf,
    pub debug_encoded_filename: Option<PathBuf>,
}

impl FileInputOutput {
    pub fn new_from_unencoded(
        unencoded_filename: &str,
        encoded_filename: Option<&str>,
        debug: bool,
    ) -> Self {
        let unencoded_filename = PathBuf::from(unencoded_filename);
        let encoded_filename = match encoded_filename {
            Some(provided) => PathBuf::from(provided),
            None => unencoded_filename.with_extension("lizard"),
        };
        let debug_encoded_filename = match debug {
            true => Some(unencoded_filename.with_extension("dblzd")),
            false => None,
        };
        FileInputOutput {
            unencoded_filename,
            encoded_filename,
            debug_encoded_filename,
        }
    }
    pub fn new_from_encoded(encoded_filename: &str, unencoded_filename: Option<&str>) -> Self {
        let encoded_filename = PathBuf::from(encoded_filename);
        let unencoded_filename = match unencoded_filename {
            Some(provided) => PathBuf::from(provided),
            None => encoded_filename.with_extension("txt"),
        };
        FileInputOutput {
            unencoded_filename,
            encoded_filename,
            debug_encoded_filename: None,
        }
    }

    pub fn input_is_valid(&self, is_encode: bool) -> Result<(), String> {
        let input_file_path = match is_encode {
            true => self.unencoded_filename.as_path(),
            false => self.encoded_filename.as_path(),
        };
        match input_file_path.exists() {
            true => Ok(()),
            false => Err(format!("Input file does not exist: {:?}", input_file_path)),
        }
    }
    pub fn output_is_valid(&self, is_encode: bool, overwrite: bool) -> Result<(), &str> {
        let output_file_path = match is_encode {
            true => self.encoded_filename.as_path(),
            false => self.unencoded_filename.as_path(),
        };
        match (output_file_path.exists(), overwrite) {
            (true, true) => {
                info!("Output file exists, but overwriting");
                Ok(())
            }
            (false, _) => Ok(()),
            (true, false) => Err("Output file exists. Consider passing overwrite to ignore this"),
        }
    }
}

mod test {
    use crate::file_io::FileInputOutput;
    use std::path::PathBuf;

    #[test]
    fn test_encoding() {
        let encoding_io = FileInputOutput::new_from_unencoded("file.txt", None, true);
        assert_eq!(encoding_io.unencoded_filename, PathBuf::from("file.txt"));
        assert_eq!(
            encoding_io.debug_encoded_filename,
            Some(PathBuf::from("file.dblzd"))
        );
        assert_eq!(encoding_io.encoded_filename, PathBuf::from("file.lizard"));

        let encoding_io =
            FileInputOutput::new_from_unencoded("file.txt", Some("custom_output.foo"), false);
        assert_eq!(encoding_io.unencoded_filename, PathBuf::from("file.txt"));
        assert_eq!(encoding_io.debug_encoded_filename, None);
        assert_eq!(
            encoding_io.encoded_filename,
            PathBuf::from("custom_output.foo")
        );
    }

    #[test]
    fn test_decoding() {
        let decoding_io = FileInputOutput::new_from_encoded("file.lizard", None);
        assert_eq!(decoding_io.encoded_filename, PathBuf::from("file.lizard"));
        assert_eq!(decoding_io.debug_encoded_filename, None);
        assert_eq!(decoding_io.unencoded_filename, PathBuf::from("file.txt"));

        let decoding_io =
            FileInputOutput::new_from_encoded("file.lizard", Some("my_unencoded_file.log"));
        assert_eq!(decoding_io.encoded_filename, PathBuf::from("file.lizard"));
        assert_eq!(decoding_io.debug_encoded_filename, None);
        assert_eq!(
            decoding_io.unencoded_filename,
            PathBuf::from("my_unencoded_file.log")
        );
    }
}
