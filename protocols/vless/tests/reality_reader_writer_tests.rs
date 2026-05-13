    use crate::reality::reality_reader_writer::*;

    #[test]
    fn test_reality_crypto_reader() {
        let mut buffer = SlideBuffer::new(1024);
        buffer.extend_from_slice(b"hello world");

        {
            let mut reader = RealityReader::new(&mut buffer, false);
            let mut buf = [0u8; 5];
            assert_eq!(reader.read(&mut buf).unwrap(), 5);
            assert_eq!(&buf, b"hello");
        }

        // Buffer should have consumed 5 bytes
        assert_eq!(buffer.len(), 6); // " world" remaining

        {
            let mut reader = RealityReader::new(&mut buffer, false);
            let mut buf = [0u8; 5];
            assert_eq!(reader.read(&mut buf).unwrap(), 5);
            assert_eq!(&buf[..5], b" worl");
        }

        assert_eq!(buffer.len(), 1); // "d" remaining
    }

    #[test]
    fn test_reality_crypto_reader_bufread() {
        let mut buffer = SlideBuffer::new(1024);
        buffer.extend_from_slice(b"test data");

        let mut reader = RealityReader::new(&mut buffer, false);

        let buf = reader.fill_buf().unwrap();
        assert_eq!(buf, b"test data");

        reader.consume(5);
        let buf = reader.fill_buf().unwrap();
        assert_eq!(buf, b"data");
    }

    #[test]
    fn test_reality_crypto_reader_empty_would_block() {
        let mut buffer = SlideBuffer::new(1024);
        // Empty buffer, no close_notify -> WouldBlock
        let mut reader = RealityReader::new(&mut buffer, false);
        let result = reader.fill_buf();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
    }

    #[test]
    fn test_reality_crypto_reader_close_notify_eof() {
        let mut buffer = SlideBuffer::new(1024);
        // Empty buffer, close_notify received -> Ok(&[]) (EOF)
        let mut reader = RealityReader::new(&mut buffer, true);
        let result = reader.fill_buf();
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_reality_crypto_writer() {
        let mut buffer = Vec::new();
        let mut writer = RealityWriter::new(&mut buffer);

        assert_eq!(writer.write(b"hello").unwrap(), 5);
        assert_eq!(writer.write(b" world").unwrap(), 6);
        assert_eq!(buffer.as_slice(), b"hello world");
    }
