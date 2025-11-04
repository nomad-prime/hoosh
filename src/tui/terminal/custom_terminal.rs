use ratatui::backend::{Backend, ClearType};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::prelude::Widget;
use ratatui::widgets::WidgetRef;
use ratatui::{CompletedFrame, TerminalOptions, Viewport};
use std::io;

#[derive(Debug, Hash)]
pub struct Frame<'a> {
    /// Where should the cursor be after drawing this frame?
    ///
    /// If `None`, the cursor is hidden and its position is controlled by the backend. If `Some((x,
    /// y))`, the cursor is shown and placed at `(x, y)` after the call to `Terminal::draw()`.
    pub(crate) cursor_position: Option<Position>,

    /// The area of the viewport
    pub(crate) viewport_area: Rect,

    /// The buffer that is used to draw the current frame
    pub(crate) buffer: &'a mut Buffer,
}

impl Frame<'_> {
    /// The area of the current frame
    ///
    /// This is guaranteed not to change during rendering, so may be called multiple times.
    ///
    /// If your app listens for a resize event from the backend, it should ignore the values from
    /// the event for any calculations that are used to render the current frame and use this value
    /// instead as this is the area of the buffer that is used to render the current frame.
    pub const fn area(&self) -> Rect {
        self.viewport_area
    }

    /// Render a [`WidgetRef`] to the current buffer using [`WidgetRef::render_ref`].
    ///
    /// Usually the area argument is the size of the current frame or a sub-area of the current
    /// frame (which can be obtained using [`Layout`] to split the total area).
    #[allow(clippy::needless_pass_by_value)]
    pub fn render_widget_ref<W: WidgetRef>(&mut self, widget: W, area: Rect) {
        widget.render_ref(area, self.buffer);
    }

    /// After drawing this frame, make the cursor visible and put it at the specified (x, y)
    /// coordinates. If this method is not called, the cursor will be hidden.
    ///
    /// Note that this will interfere with calls to [`Terminal::hide_cursor`],
    /// [`Terminal::show_cursor`], and [`Terminal::set_cursor_position`]. Pick one of the APIs and
    /// stick with it.
    ///
    /// [`Terminal::hide_cursor`]: crate::Terminal::hide_cursor
    /// [`Terminal::show_cursor`]: crate::Terminal::show_cursor
    /// [`Terminal::set_cursor_position`]: crate::Terminal::set_cursor_position
    pub fn set_cursor_position<P: Into<Position>>(&mut self, position: P) {
        self.cursor_position = Some(position.into());
    }

    /// Gets the buffer that this `Frame` draws into as a mutable reference.
    pub fn buffer_mut(&mut self) -> &mut Buffer {
        self.buffer
    }

    pub fn render_widget<W: Widget>(&mut self, widget: W, area: Rect) {
        widget.render(area, self.buffer);
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Terminal<B>
where
    B: Backend,
{
    /// The backend used to interface with the terminal
    backend: B,
    /// Holds the results of the current and previous draw calls. The two are compared at the end
    /// of each draw pass to output the necessary updates to the terminal
    buffers: [Buffer; 2],
    /// Index of the current buffer in the previous array
    current: usize,
    /// Whether the cursor is currently hidden
    hidden_cursor: bool,
    /// Viewport
    viewport: Viewport,
    /// Area of the viewport
    viewport_area: Rect,
    /// Last known area of the terminal. Used to detect if the internal buffers have to be resized.
    last_known_area: Rect,
    /// Last known position of the cursor. Used to find the new area when the viewport is inlined
    /// and the terminal resized.
    last_known_cursor_pos: Position,
    /// Number of frames rendered up until current time.
    frame_count: usize,
}

impl<B> Drop for Terminal<B>
where
    B: Backend,
{
    fn drop(&mut self) {
        // Attempt to restore the cursor state
        if self.hidden_cursor
            && let Err(err) = self.show_cursor()
        {
            eprintln!("Failed to show the cursor: {err}");
        }
    }
}

impl<B> Terminal<B>
where
    B: Backend,
{
    /// Creates a new [`Terminal`] with the given [`Backend`] with a full screen viewport.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::io::stdout;
    ///
    /// use ratatui::{backend::CrosstermBackend, Terminal};
    ///
    /// let backend = CrosstermBackend::new(stdout());
    /// let terminal = Terminal::new(backend)?;
    /// # std::io::Result::Ok(())
    /// ```
    pub fn new(backend: B) -> io::Result<Self> {
        Self::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Fullscreen,
            },
        )
    }

    /// Creates a new [`Terminal`] with the given [`Backend`] and [`TerminalOptions`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::io::stdout;
    ///
    /// use ratatui::{backend::CrosstermBackend, layout::Rect, Terminal, TerminalOptions, Viewport};
    ///
    /// let backend = CrosstermBackend::new(stdout());
    /// let viewport = Viewport::Fixed(Rect::new(0, 0, 10, 10));
    /// let terminal = Terminal::with_options(backend, TerminalOptions { viewport })?;
    /// # std::io::Result::Ok(())
    /// ```
    pub fn with_options(mut backend: B, options: TerminalOptions) -> io::Result<Self> {
        let area = match options.viewport {
            Viewport::Fullscreen | Viewport::Inline(_) => {
                Rect::from((Position::ORIGIN, backend.size()?))
            }
            Viewport::Fixed(area) => area,
        };
        let (viewport_area, cursor_pos) = match options.viewport {
            Viewport::Fullscreen => (area, Position::ORIGIN),
            Viewport::Inline(height) => {
                compute_inline_size(&mut backend, height, area.as_size(), 0)?
            }
            Viewport::Fixed(area) => (area, area.as_position()),
        };
        Ok(Self {
            backend,
            buffers: [Buffer::empty(viewport_area), Buffer::empty(viewport_area)],
            current: 0,
            hidden_cursor: false,
            viewport: options.viewport,
            viewport_area,
            last_known_area: area,
            last_known_cursor_pos: cursor_pos,
            frame_count: 0,
        })
    }

    /// Get a Frame object which provides a consistent view into the terminal state for rendering.
    pub fn get_frame(&mut self) -> Frame<'_> {
        Frame {
            cursor_position: None,
            viewport_area: self.viewport_area,
            buffer: self.current_buffer_mut(),
        }
    }

    /// Gets the current buffer as a mutable reference.
    pub fn current_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.current]
    }

    /// Gets the backend
    pub const fn backend(&self) -> &B {
        &self.backend
    }

    /// Gets the backend as a mutable reference
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Obtains a difference between the previous and the current buffer and passes it to the
    /// current backend for drawing.
    pub fn flush(&mut self) -> io::Result<()> {
        let previous_buffer = &self.buffers[1 - self.current];
        let current_buffer = &self.buffers[self.current];
        let updates = previous_buffer.diff(current_buffer);
        if let Some((col, row, _)) = updates.last() {
            self.last_known_cursor_pos = Position { x: *col, y: *row };
        }
        self.backend.draw(updates.into_iter())
    }

    /// Updates the Terminal so that internal buffers match the requested area.
    ///
    /// Requested area will be saved to remain consistent when rendering. This leads to a full clear
    /// of the screen.
    pub fn resize(&mut self, area: Rect) -> io::Result<()> {
        let next_area = match self.viewport {
            Viewport::Inline(height) => {
                let offset_in_previous_viewport = self
                    .last_known_cursor_pos
                    .y
                    .saturating_sub(self.viewport_area.top());
                compute_inline_size(
                    &mut self.backend,
                    height,
                    area.as_size(),
                    offset_in_previous_viewport,
                )?
                .0
            }
            Viewport::Fixed(_) | Viewport::Fullscreen => area,
        };
        self.set_viewport_area(next_area);
        self.clear()?;

        self.last_known_area = area;
        Ok(())
    }

    pub fn set_viewport_area(&mut self, area: Rect) {
        self.buffers[self.current].resize(area);
        self.buffers[1 - self.current].resize(area);
        self.viewport_area = area;
    }

    pub fn get_viewport_area(&self) -> Rect {
        self.viewport_area
    }

    /// Queries the backend for size and resizes if it doesn't match the previous size.
    pub fn autoresize(&mut self) -> io::Result<()> {
        // fixed viewports do not get autoresized
        if matches!(self.viewport, Viewport::Fullscreen | Viewport::Inline(_)) {
            let area = Rect::from((Position::ORIGIN, self.size()?));
            if area != self.last_known_area {
                self.resize(area)?;
            }
        };
        Ok(())
    }

    /// Draws a single frame to the terminal.
    ///
    /// Returns a [`CompletedFrame`] if successful, otherwise a [`std::io::Error`].
    ///
    /// If the render callback passed to this method can fail, use [`try_draw`] instead.
    ///
    /// Applications should call `draw` or [`try_draw`] in a loop to continuously render the
    /// terminal. These methods are the main entry points for drawing to the terminal.
    ///
    /// [`try_draw`]: Terminal::try_draw
    ///
    /// This method will:
    ///
    /// - autoresize the terminal if necessary
    /// - call the render callback, passing it a [`Frame`] reference to render to
    /// - flush the current internal state by copying the current buffer to the backend
    /// - move the cursor to the last known position if it was set during the rendering closure
    /// - return a [`CompletedFrame`] with the current buffer and the area of the terminal
    ///
    /// The [`CompletedFrame`] returned by this method can be useful for debugging or testing
    /// purposes, but it is often not used in regular applicationss.
    ///
    /// The render callback should fully render the entire frame when called, including areas that
    /// are unchanged from the previous frame. This is because each frame is compared to the
    /// previous frame to determine what has changed, and only the changes are written to the
    /// terminal. If the render callback does not fully render the frame, the terminal will not be
    /// in a consistent state.
    ///
    /// # Examples
    ///
    /// ```
    /// # let backend = ratatui::backend::TestBackend::new(10, 10);
    /// # let mut terminal = ratatui::Terminal::new(backend)?;
    /// use ratatui::{layout::Position, widgets::Paragraph};
    ///
    /// // with a closure
    /// terminal.draw(|frame| {
    ///     let area = frame.area();
    ///     frame.render_widget(Paragraph::new("Hello World!"), area);
    ///     frame.set_cursor_position(Position { x: 0, y: 0 });
    /// })?;
    ///
    /// // or with a function
    /// terminal.draw(render)?;
    ///
    /// fn render(frame: &mut ratatui::Frame) {
    ///     frame.render_widget(Paragraph::new("Hello World!"), frame.area());
    /// }
    /// # std::io::Result::Ok(())
    /// ```
    pub fn draw<F>(&mut self, render_callback: F) -> io::Result<CompletedFrame<'_>>
    where
        F: FnOnce(&mut Frame),
    {
        self.try_draw(|frame| {
            render_callback(frame);
            io::Result::Ok(())
        })
    }

    /// Tries to draw a single frame to the terminal.
    ///
    /// Returns [`Result::Ok`] containing a [`CompletedFrame`] if successful, otherwise
    /// [`Result::Err`] containing the [`std::io::Error`] that caused the failure.
    ///
    /// This is the equivalent of [`Terminal::draw`] but the render callback is a function or
    /// closure that returns a `Result` instead of nothing.
    ///
    /// Applications should call `try_draw` or [`draw`] in a loop to continuously render the
    /// terminal. These methods are the main entry points for drawing to the terminal.
    ///
    /// [`draw`]: Terminal::draw
    ///
    /// This method will:
    ///
    /// - autoresize the terminal if necessary
    /// - call the render callback, passing it a [`Frame`] reference to render to
    /// - flush the current internal state by copying the current buffer to the backend
    /// - move the cursor to the last known position if it was set during the rendering closure
    /// - return a [`CompletedFrame`] with the current buffer and the area of the terminal
    ///
    /// The render callback passed to `try_draw` can return any [`Result`] with an error type that
    /// can be converted into an [`std::io::Error`] using the [`Into`] trait. This makes it possible
    /// to use the `?` operator to propagate errors that occur during rendering. If the render
    /// callback returns an error, the error will be returned from `try_draw` as an
    /// [`std::io::Error`] and the terminal will not be updated.
    ///
    /// The [`CompletedFrame`] returned by this method can be useful for debugging or testing
    /// purposes, but it is often not used in regular applicationss.
    ///
    /// The render callback should fully render the entire frame when called, including areas that
    /// are unchanged from the previous frame. This is because each frame is compared to the
    /// previous frame to determine what has changed, and only the changes are written to the
    /// terminal. If the render function does not fully render the frame, the terminal will not be
    /// in a consistent state.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// # use ratatui::layout::Position;;
    /// # let backend = ratatui::backend::TestBackend::new(10, 10);
    /// # let mut terminal = ratatui::Terminal::new(backend)?;
    /// use std::io;
    ///
    /// use ratatui::widgets::Paragraph;
    ///
    /// // with a closure
    /// terminal.try_draw(|frame| {
    ///     let value: u8 = "not a number".parse().map_err(io::Error::other)?;
    ///     let area = frame.area();
    ///     frame.render_widget(Paragraph::new("Hello World!"), area);
    ///     frame.set_cursor_position(Position { x: 0, y: 0 });
    ///     io::Result::Ok(())
    /// })?;
    ///
    /// // or with a function
    /// terminal.try_draw(render)?;
    ///
    /// fn render(frame: &mut ratatui::Frame) -> io::Result<()> {
    ///     let value: u8 = "not a number".parse().map_err(io::Error::other)?;
    ///     frame.render_widget(Paragraph::new("Hello World!"), frame.area());
    ///     Ok(())
    /// }
    /// # io::Result::Ok(())
    /// ```
    pub fn try_draw<F, E>(&mut self, render_callback: F) -> io::Result<CompletedFrame<'_>>
    where
        F: FnOnce(&mut Frame) -> Result<(), E>,
        E: Into<io::Error>,
    {
        // Autoresize - otherwise we get glitches if shrinking or potential desync between widgets
        // and the terminal (if growing), which may OOB.
        self.autoresize()?;

        let mut frame = self.get_frame();

        render_callback(&mut frame).map_err(Into::into)?;

        // We can't change the cursor position right away because we have to flush the frame to
        // stdout first. But we also can't keep the frame around, since it holds a &mut to
        // Buffer. Thus, we're taking the important data out of the Frame and dropping it.
        let cursor_position = frame.cursor_position;

        // Draw to stdout
        self.flush()?;

        match cursor_position {
            None => self.hide_cursor()?,
            Some(position) => {
                self.show_cursor()?;
                self.set_cursor_position(position)?;
            }
        }

        self.swap_buffers();

        // Flush
        self.backend.flush()?;

        let completed_frame = CompletedFrame {
            buffer: &self.buffers[1 - self.current],
            area: self.last_known_area,
            count: self.frame_count,
        };

        // increment frame count before returning from draw
        self.frame_count = self.frame_count.wrapping_add(1);

        Ok(completed_frame)
    }

    /// Hides the cursor.
    pub fn hide_cursor(&mut self) -> io::Result<()> {
        self.backend.hide_cursor()?;
        self.hidden_cursor = true;
        Ok(())
    }

    /// Shows the cursor.
    pub fn show_cursor(&mut self) -> io::Result<()> {
        self.backend.show_cursor()?;
        self.hidden_cursor = false;
        Ok(())
    }

    /// Gets the current cursor position.
    ///
    /// This is the position of the cursor after the last draw call and is returned as a tuple of
    /// `(x, y)` coordinates.
    #[deprecated = "the method get_cursor_position indicates more clearly what about the cursor to get"]
    pub fn get_cursor(&mut self) -> io::Result<(u16, u16)> {
        let Position { x, y } = self.get_cursor_position()?;
        Ok((x, y))
    }

    /// Sets the cursor position.
    #[deprecated = "the method set_cursor_position indicates more clearly what about the cursor to set"]
    pub fn set_cursor(&mut self, x: u16, y: u16) -> io::Result<()> {
        self.set_cursor_position(Position { x, y })
    }

    /// Gets the current cursor position.
    ///
    /// This is the position of the cursor after the last draw call.
    pub fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.backend.get_cursor_position()
    }

    /// Sets the cursor position.
    pub fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let position = position.into();
        self.backend.set_cursor_position(position)?;
        self.last_known_cursor_pos = position;
        Ok(())
    }

    /// Clear the terminal and force a full redraw on the next draw call.
    pub fn clear(&mut self) -> io::Result<()> {
        match self.viewport {
            Viewport::Fullscreen => self.backend.clear_region(ClearType::All)?,
            Viewport::Inline(_) => {
                self.backend
                    .set_cursor_position(self.viewport_area.as_position())?;
                self.backend.clear_region(ClearType::AfterCursor)?;
            }
            Viewport::Fixed(_) => {
                let area = self.viewport_area;
                for y in area.top()..area.bottom() {
                    self.backend.set_cursor_position(Position { x: 0, y })?;
                    self.backend.clear_region(ClearType::AfterCursor)?;
                }
            }
        }
        // Reset the back buffer to make sure the next update will redraw everything.
        self.buffers[1 - self.current].reset();
        Ok(())
    }

    /// Clears the inactive buffer and swaps it with the current buffer
    pub fn swap_buffers(&mut self) {
        self.buffers[1 - self.current].reset();
        self.current = 1 - self.current;
    }

    /// Queries the real size of the backend.
    pub fn size(&self) -> io::Result<Size> {
        self.backend.size()
    }

    /// Insert some content before the current inline viewport. This has no effect when the
    /// viewport is not inline.
    ///
    /// The `draw_fn` closure will be called to draw into a writable `Buffer` that is `height`
    /// lines tall. The content of that `Buffer` will then be inserted before the viewport.
    ///
    /// If the viewport isn't yet at the bottom of the screen, inserted lines will push it towards
    /// the bottom. Once the viewport is at the bottom of the screen, inserted lines will scroll
    /// the area of the screen above the viewport upwards.
    ///
    /// Before:
    /// ```ignore
    /// +---------------------+
    /// | pre-existing line 1 |
    /// | pre-existing line 2 |
    /// +---------------------+
    /// |       viewport      |
    /// +---------------------+
    /// |                     |
    /// |                     |
    /// +---------------------+
    /// ```
    ///
    /// After inserting 2 lines:
    /// ```ignore
    /// +---------------------+
    /// | pre-existing line 1 |
    /// | pre-existing line 2 |
    /// |   inserted line 1   |
    /// |   inserted line 2   |
    /// +---------------------+
    /// |       viewport      |
    /// +---------------------+
    /// +---------------------+
    /// ```
    ///
    /// After inserting 2 more lines:
    /// ```ignore
    /// +---------------------+
    /// | pre-existing line 2 |
    /// |   inserted line 1   |
    /// |   inserted line 2   |
    /// |   inserted line 3   |
    /// |   inserted line 4   |
    /// +---------------------+
    /// |       viewport      |
    /// +---------------------+
    /// ```
    ///
    /// If more lines are inserted than there is space on the screen, then the top lines will go
    /// directly into the terminal's scrollback buffer. At the limit, if the viewport takes up the
    /// whole screen, all lines will be inserted directly into the scrollback buffer.
    ///
    /// # Examples
    ///
    /// ## Insert a single line before the current viewport
    ///
    pub fn insert_before<F>(&mut self, height: u16, draw_fn: F) -> io::Result<()>
    where
        F: FnOnce(&mut Buffer),
    {
        match self.viewport {
            Viewport::Inline(_) => self.insert_before_scrolling_regions(height, draw_fn),
            _ => Ok(()),
        }
    }

    fn insert_before_scrolling_regions(
        &mut self,
        mut height: u16,
        draw_fn: impl FnOnce(&mut Buffer),
    ) -> io::Result<()> {
        // The approach of this function is to first render all of the lines to insert into a
        // temporary buffer, and then to loop drawing chunks from the buffer to the screen. drawing
        // this buffer onto the screen.
        let area = Rect {
            x: 0,
            y: 0,
            width: self.viewport_area.width,
            height,
        };
        let mut buffer = Buffer::empty(area);
        draw_fn(&mut buffer);
        let mut buffer = buffer.content.as_slice();

        // Handle the special case where the viewport takes up the whole screen.
        if self.viewport_area.height == self.last_known_area.height {
            // "Borrow" the top line of the viewport. Draw over it, then immediately scroll it into
            // scrollback. Do this repeatedly until the whole buffer has been put into scrollback.
            let mut first = true;
            while !buffer.is_empty() {
                buffer = if first {
                    self.draw_lines(0, 1, buffer)?
                } else {
                    self.draw_lines_over_cleared(0, 1, buffer)?
                };
                first = false;
                self.backend.scroll_region_up(0..1, 1)?;
            }

            // Redraw the top line of the viewport.
            let width = self.viewport_area.width as usize;
            let top_line = self.buffers[1 - self.current].content[0..width].to_vec();
            self.draw_lines_over_cleared(0, 1, &top_line)?;
            return Ok(());
        }

        // Handle the case where the viewport isn't yet at the bottom of the screen.
        {
            let viewport_top = self.viewport_area.top();
            let viewport_bottom = self.viewport_area.bottom();
            let screen_bottom = self.last_known_area.bottom();
            if viewport_bottom < screen_bottom {
                let to_draw = height.min(screen_bottom - viewport_bottom);
                self.backend
                    .scroll_region_down(viewport_top..viewport_bottom + to_draw, to_draw)?;
                buffer = self.draw_lines_over_cleared(viewport_top, to_draw, buffer)?;
                self.set_viewport_area(Rect {
                    y: viewport_top + to_draw,
                    ..self.viewport_area
                });
                height -= to_draw;
            }
        }

        let viewport_top = self.viewport_area.top();
        while height > 0 {
            let to_draw = height.min(viewport_top);
            self.backend.scroll_region_up(0..viewport_top, to_draw)?;
            buffer = self.draw_lines_over_cleared(viewport_top - to_draw, to_draw, buffer)?;
            height -= to_draw;
        }

        Ok(())
    }

    /// Draw lines at the given vertical offset. The slice of cells must contain enough cells
    /// for the requested lines. A slice of the unused cells are returned.
    fn draw_lines<'a>(
        &mut self,
        y_offset: u16,
        lines_to_draw: u16,
        cells: &'a [Cell],
    ) -> io::Result<&'a [Cell]> {
        let width: usize = self.last_known_area.width.into();
        let (to_draw, remainder) = cells.split_at(width * lines_to_draw as usize);
        if lines_to_draw > 0 {
            let iter = to_draw
                .iter()
                .enumerate()
                .map(|(i, c)| ((i % width) as u16, y_offset + (i / width) as u16, c));
            self.backend.draw(iter)?;
            self.backend.flush()?;
        }
        Ok(remainder)
    }

    /// Draw lines at the given vertical offset, assuming that the lines they are replacing on the
    /// screen are cleared. The slice of cells must contain enough cells for the requested lines. A
    /// slice of the unused cells are returned.
    fn draw_lines_over_cleared<'a>(
        &mut self,
        y_offset: u16,
        lines_to_draw: u16,
        cells: &'a [Cell],
    ) -> io::Result<&'a [Cell]> {
        let width: usize = self.last_known_area.width.into();
        let (to_draw, remainder) = cells.split_at(width * lines_to_draw as usize);
        if lines_to_draw > 0 {
            let area = Rect::new(0, y_offset, width as u16, y_offset + lines_to_draw);
            let old = Buffer::empty(area);
            let new = Buffer {
                area,
                content: to_draw.to_vec(),
            };
            self.backend.draw(old.diff(&new).into_iter())?;
            self.backend.flush()?;
        }
        Ok(remainder)
    }
}

fn compute_inline_size<B: Backend>(
    backend: &mut B,
    height: u16,
    size: Size,
    offset_in_previous_viewport: u16,
) -> io::Result<(Rect, Position)> {
    let pos = backend.get_cursor_position()?;
    let mut row = pos.y;

    let max_height = size.height.min(height);

    let lines_after_cursor = height
        .saturating_sub(offset_in_previous_viewport)
        .saturating_sub(1);

    backend.append_lines(lines_after_cursor)?;

    let available_lines = size.height.saturating_sub(row).saturating_sub(1);
    let missing_lines = lines_after_cursor.saturating_sub(available_lines);
    if missing_lines > 0 {
        row = row.saturating_sub(missing_lines);
    }
    row = row.saturating_sub(offset_in_previous_viewport);

    Ok((
        Rect {
            x: 0,
            y: row,
            width: size.width,
            height: max_height,
        },
        pos,
    ))
}
