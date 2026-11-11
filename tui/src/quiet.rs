//! A terminal backend that writes nothing for a frame in which no cell changed.

use std::io::{self, Stdout};

use ratatui::backend::{Backend, ClearType, CrosstermBackend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use ratatui::Terminal;

/// The dashboard's terminal.
pub type Term = Terminal<Quiet<CrosstermBackend<Stdout>>>;

/// Set up the terminal as `ratatui::init` does, on the quiet backend.
pub fn init() -> Term {
    drop(ratatui::init());
    Terminal::new(Quiet::new(CrosstermBackend::new(io::stdout())))
        .expect("the terminal answers once ratatui::init has set it up")
}

/// Wraps a backend: an empty frame and the cursor hide that follows it reach the terminal as nothing.
pub struct Quiet<B> {
    inner: B,
    idle: bool,
}

impl<B> Quiet<B> {
    pub fn new(inner: B) -> Self {
        Self { inner, idle: false }
    }
}

impl<B: Backend> Backend for Quiet<B> {
    type Error = B::Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut content = content.peekable();
        self.idle = content.peek().is_none();
        if self.idle {
            return Ok(());
        }
        self.inner.draw(content)
    }

    fn append_lines(&mut self, n: u16) -> Result<(), Self::Error> {
        self.inner.append_lines(n)
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        if self.idle {
            return Ok(());
        }
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.idle = false;
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> Result<Size, Self::Error> {
        self.inner.size()
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Paragraph;
    use std::cell::RefCell;
    use std::rc::Rc;

    type Bytes = Rc<RefCell<Vec<u8>>>;

    struct Sink(Bytes);

    impl io::Write for Sink {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct Fixed {
        size: Size,
        out: CrosstermBackend<Sink>,
    }

    impl Backend for Fixed {
        type Error = io::Error;
        fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
        where
            I: Iterator<Item = (u16, u16, &'a Cell)>,
        {
            self.out.draw(content)
        }
        fn hide_cursor(&mut self) -> io::Result<()> {
            self.out.hide_cursor()
        }
        fn show_cursor(&mut self) -> io::Result<()> {
            self.out.show_cursor()
        }
        fn get_cursor_position(&mut self) -> io::Result<Position> {
            Ok(Position::ORIGIN)
        }
        fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
            self.out.set_cursor_position(position)
        }
        fn clear(&mut self) -> io::Result<()> {
            self.out.clear()
        }
        fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
            self.out.clear_region(clear_type)
        }
        fn size(&self) -> io::Result<Size> {
            Ok(self.size)
        }
        fn window_size(&mut self) -> io::Result<WindowSize> {
            Ok(WindowSize {
                columns_rows: self.size,
                pixels: Size::default(),
            })
        }
        fn flush(&mut self) -> io::Result<()> {
            Backend::flush(&mut self.out)
        }
    }

    fn terminal() -> (Terminal<Quiet<Fixed>>, Bytes) {
        let bytes = Bytes::default();
        let fixed = Fixed {
            size: Size::new(20, 3),
            out: CrosstermBackend::new(Sink(Rc::clone(&bytes))),
        };
        (Terminal::new(Quiet::new(fixed)).unwrap(), bytes)
    }

    fn taken(bytes: &Bytes) -> String {
        String::from_utf8_lossy(&std::mem::take(&mut *bytes.borrow_mut())).into_owned()
    }

    fn show(t: &mut Terminal<Quiet<Fixed>>, text: &str) {
        t.draw(|f| f.render_widget(Paragraph::new(text), f.area()))
            .unwrap();
    }

    #[test]
    fn an_unchanged_frame_writes_nothing() {
        let (mut t, bytes) = terminal();
        show(&mut t, "hello");
        assert!(taken(&bytes).contains("hello"));
        show(&mut t, "hello");
        assert_eq!(taken(&bytes), "");
    }

    #[test]
    fn a_changed_frame_is_written_and_hides_the_cursor() {
        let (mut t, bytes) = terminal();
        show(&mut t, "hello");
        show(&mut t, "hello");
        taken(&bytes);
        show(&mut t, "world");
        let out = taken(&bytes);
        assert!(out.contains("w"), "{out:?}");
        assert!(out.ends_with("\x1b[?25l"), "{out:?}");
    }

    #[test]
    fn a_cursor_hide_after_a_show_reaches_the_terminal() {
        let (mut t, bytes) = terminal();
        show(&mut t, "hello");
        show(&mut t, "hello");
        taken(&bytes);
        t.show_cursor().unwrap();
        t.hide_cursor().unwrap();
        assert!(taken(&bytes).ends_with("\x1b[?25l"));
    }

    #[test]
    fn the_test_backend_sees_every_changed_frame() {
        let mut t = Terminal::new(Quiet::new(TestBackend::new(10, 1))).unwrap();
        t.draw(|f| f.render_widget(Paragraph::new("one"), f.area()))
            .unwrap();
        t.draw(|f| f.render_widget(Paragraph::new("two"), f.area()))
            .unwrap();
        t.backend().inner.assert_buffer_lines(["two       "]);
    }
}
