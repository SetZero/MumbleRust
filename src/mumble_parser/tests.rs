#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio::io::{AsyncRead, BufStream, ReadBuf};
    use crate::MumbleParser;

    struct Mock {

    }

    impl Mock {
        fn new() -> Mock {
            Mock{}
        }
    }

    impl AsyncRead for Mock {
        fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
            todo!()
        }
    }

    # [test]
    fn it_works() {
        let buffer = Mock::new();
        MumbleParser::new(buffer);
        assert_eq!(1+1, 2);
    }
}