    use crate::reality::slide_buffer::*;

    #[test]
    fn test_new_buffer() {
        let buf = SlideBuffer::new(1024);
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());
        assert_eq!(buf.remaining_capacity(), 1024);
    }

    #[test]
    fn test_is_empty() {
        let mut buf = SlideBuffer::new(1024);
        assert!(buf.is_empty());

        buf.extend_from_slice(b"hello");
        assert!(!buf.is_empty());

        buf.consume(3);
        assert!(!buf.is_empty());

        buf.consume(2);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_extend_from_slice() {
        let mut buf = SlideBuffer::new(1024);
        buf.extend_from_slice(b"hello");
        assert_eq!(buf.len(), 5);
        assert_eq!(buf.as_slice(), b"hello");
        assert_eq!(buf.remaining_capacity(), 1024 - 5);
    }

    #[test]
    fn test_consume() {
        let mut buf = SlideBuffer::new(1024);
        buf.extend_from_slice(b"hello world");
        buf.consume(6);
        assert_eq!(buf.as_slice(), b"world");
        assert_eq!(buf.len(), 5);
    }

    #[test]
    fn test_consume_all_resets() {
        let mut buf = SlideBuffer::new(1024);
        buf.extend_from_slice(b"hello");
        buf.consume(5);
        assert_eq!(buf.len(), 0);
        assert_eq!(buf.remaining_capacity(), 1024);
    }

    #[test]
    fn test_compact() {
        let mut buf = SlideBuffer::new(1024);
        buf.extend_from_slice(b"hello world");
        buf.consume(6);
        assert_eq!(buf.remaining_capacity(), 1024 - 11);

        buf.compact();
        assert_eq!(buf.as_slice(), b"world");
        assert_eq!(buf.remaining_capacity(), 1024 - 5);
    }

    #[test]
    fn test_maybe_compact() {
        let mut buf = SlideBuffer::new(1024);
        buf.extend_from_slice(b"0123456789"); // 10 bytes
        buf.consume(5);

        // Threshold not met
        buf.maybe_compact(10);
        assert_eq!(buf.remaining_capacity(), 1024 - 10);

        // Threshold met
        buf.maybe_compact(4);
        assert_eq!(buf.remaining_capacity(), 1024 - 5);
    }

    #[test]
    fn test_write_slice() {
        let mut buf = SlideBuffer::new(1024);
        let write_buf = buf.write_slice();
        write_buf[..5].copy_from_slice(b"hello");
        buf.advance_write(5);

        assert_eq!(buf.as_slice(), b"hello");
        assert_eq!(buf.len(), 5);
    }

    #[test]
    fn test_read_trait() {
        let mut buf = SlideBuffer::new(1024);
        buf.extend_from_slice(b"hello world");

        let mut output = [0u8; 5];
        let n = buf.read(&mut output).unwrap();
        assert_eq!(n, 5);
        assert_eq!(&output, b"hello");

        let n = buf.read(&mut output).unwrap();
        assert_eq!(n, 5);
        assert_eq!(&output, b" worl");

        let n = buf.read(&mut output).unwrap();
        assert_eq!(n, 1);
        assert_eq!(&output[..1], b"d");
    }

    #[test]
    fn test_bufread_trait() {
        let mut buf = SlideBuffer::new(1024);
        buf.extend_from_slice(b"hello world");

        {
            let slice = buf.fill_buf().unwrap();
            assert_eq!(slice, b"hello world");
        }

        buf.consume(6);

        {
            let slice = buf.fill_buf().unwrap();
            assert_eq!(slice, b"world");
        }
    }

    #[test]
    fn test_indexing() {
        let mut buf = SlideBuffer::new(1024);
        buf.extend_from_slice(b"hello world");

        assert_eq!(buf[0], b'h');
        assert_eq!(buf[6], b'w');
        assert_eq!(&buf[0..5], b"hello");
        assert_eq!(&buf[6..], b"world");
        assert_eq!(&buf[..5], b"hello");
    }

    #[test]
    fn test_indexing_after_consume() {
        let mut buf = SlideBuffer::new(1024);
        buf.extend_from_slice(b"hello world");
        buf.consume(6);

        assert_eq!(buf[0], b'w');
        assert_eq!(&buf[0..5], b"world");
    }

    #[test]
    fn test_get_u16_be() {
        let mut buf = SlideBuffer::new(1024);
        buf.extend_from_slice(&[0x12, 0x34, 0x56, 0x78]);

        assert_eq!(buf.get_u16_be(0), Some(0x1234));
        assert_eq!(buf.get_u16_be(2), Some(0x5678));
        assert_eq!(buf.get_u16_be(3), None);
    }

    #[test]
    fn test_multiple_extend_consume_cycles() {
        let mut buf = SlideBuffer::new(100);

        for i in 0..10 {
            buf.extend_from_slice(b"0123456789");
            assert_eq!(buf.len(), 10);
            buf.consume(10);
            assert_eq!(buf.len(), 0);
            assert_eq!(buf.remaining_capacity(), 100, "iteration {} failed", i);
        }
    }

    #[test]
    fn test_partial_consume_and_extend() {
        let mut buf = SlideBuffer::new(100);

        buf.extend_from_slice(b"hello world"); // 11 bytes
        buf.consume(6); // consume "hello "

        buf.extend_from_slice(b"!!!"); // add "!!!"
        assert_eq!(buf.as_slice(), b"world!!!");

        buf.compact();
        assert_eq!(buf.as_slice(), b"world!!!");
        assert_eq!(buf.remaining_capacity(), 100 - 8);
    }

    #[test]
    fn test_slice_mut() {
        let mut buf = SlideBuffer::new(100);
        buf.extend_from_slice(b"hello world");

        // Modify middle portion
        let slice = buf.slice_mut(6..11);
        assert_eq!(slice, b"world");
        slice[0] = b'W';
        slice[4] = b'D';

        assert_eq!(buf.as_slice(), b"hello WorlD");

        // Test after consume
        buf.consume(6);
        let slice = buf.slice_mut(0..5);
        slice.copy_from_slice(b"EARTH");
        assert_eq!(buf.as_slice(), b"EARTH");
    }
