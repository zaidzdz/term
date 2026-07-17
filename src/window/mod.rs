use crate::pty;
use crate::renderer::RenderContext;
use ansi_escape_sequences::strip_ansi;
use ansi_parser::{AnsiParser, AnsiSequence, Output};
use nix::libc::winsize;
use pty::PtyContext;
use std::sync::Arc;
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;
pub struct TermWindow {
    pub window: Arc<Window>,
    pty: PtyContext,
    buffer: Vec<String>, // each line is one element
    current_row: String, // partial line being built
    render_context: RenderContext,
    cursor_x: u32,
    cursor_y: u32,
}

impl TermWindow {
    pub async fn new(event_loop: &EventLoop<()>, pty: PtyContext) -> Self {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("term"))
                .expect("Could not create window"),
        );
        let render_context = RenderContext::new(window.clone()).await;
        Self {
            window,
            pty,
            buffer: Vec::new(),
            current_row: String::new(),
            render_context,
            cursor_x: 0,
            cursor_y: 0,
        }
    }

    pub fn run(mut self, event_loop: EventLoop<()>) {
        #[allow(deprecated)]
        event_loop
            .run(move |event, elwt| {
                elwt.set_control_flow(ControlFlow::Poll);

                match event {
                    Event::WindowEvent { event, window_id } if window_id == self.window.id() => {
                        match event {
                            WindowEvent::CloseRequested => elwt.exit(),
                            WindowEvent::Resized(size) => {
                                self.render_context.resize(size.width, size.height);
                                self.pty
                                    .resize(&winsize {
                                        ws_row: (size.height / 20) as u16,
                                        ws_col: (size.width / 8) as u16,
                                        ws_xpixel: size.width as u16,
                                        ws_ypixel: size.height as u16,
                                    })
                                    .unwrap();
                            }
                            WindowEvent::RedrawRequested => {
                                self.render_context.render().unwrap();
                            }
                            WindowEvent::KeyboardInput { event, .. } => {
                                if event.state == ElementState::Pressed {
                                    if let Some(text) = &event.text {
                                        self.pty
                                            .write(text.as_bytes())
                                            .expect("Could not write to pty");
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Event::AboutToWait => {
                        loop {
                            match self.pty.rx.try_recv() {
                                Ok(data) => {
                                    let text = String::from_utf8_lossy(&data);
                                    let parsed: Vec<Output> = text.as_ref().ansi_parse().collect();
                                    for block in parsed.into_iter() {
                                        match block {
                                            Output::TextBlock(text) => {
                                                let mut in_escape = false;

                                                for ch in text.chars() {
                                                    if ch == '\x1b' {
                                                        in_escape = true;
                                                        continue;
                                                    }
                                                    if in_escape {
                                                        if ch.is_ascii_alphabetic() {
                                                            in_escape = false;
                                                        }
                                                        continue;
                                                    }

                                                    match ch {
                                                        '\r' => {
                                                            self.cursor_x = 0;
                                                        }
                                                        '\n' => {
                                                            self.cursor_y += 1;
                                                            self.buffer.push(std::mem::take(
                                                                &mut self.current_row,
                                                            ));
                                                        }
                                                        _ => {
                                                            if ch >= ' ' {
                                                                let x = self.cursor_x as usize;
                                                                match self
                                                                    .current_row
                                                                    .char_indices()
                                                                    .nth(x)
                                                                {
                                                                    Some((byte_idx, existing)) => {
                                                                        let end = byte_idx
                                                                            + existing.len_utf8();
                                                                        self.current_row
                                                                            .replace_range(
                                                                                byte_idx..end,
                                                                                &ch.to_string(),
                                                                            );
                                                                    }
                                                                    None => {
                                                                        self.current_row.push(ch)
                                                                    }
                                                                }
                                                                self.cursor_x += 1;
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            Output::Escape(seq) => match seq {
                                                AnsiSequence::CursorUp(n) => {
                                                    self.cursor_y =
                                                        self.cursor_y.saturating_sub(n as u32)
                                                }
                                                AnsiSequence::CursorDown(n) => {
                                                    self.cursor_y += n as u32
                                                }
                                                AnsiSequence::CursorForward(n) => {
                                                    self.cursor_x += n as u32
                                                }
                                                AnsiSequence::CursorBackward(n) => {
                                                    self.cursor_x =
                                                        self.cursor_x.saturating_sub(n as u32)
                                                }
                                                AnsiSequence::CursorPos(row, col) => {
                                                    self.cursor_y = row.saturating_sub(1) as u32;
                                                    self.cursor_x = col.saturating_sub(1) as u32;
                                                }
                                                AnsiSequence::EraseDisplay => {
                                                    println!("DIAGAGADGAGSdsfsdfsdf ");
                                                    self.buffer.clear();
                                                    self.current_row.clear();
                                                }
                                                _ => {}
                                            },
                                        }
                                    }
                                    let mut fin = String::new();
                                    for str in &self.buffer {
                                        fin.push_str(str);
                                        fin.push('\n');
                                    }
                                    fin.push_str(&self.current_row);

                                    self.render_context.set_text(&strip_ansi(fin.as_ref()));
                                    self.render_context
                                        .set_cursor_pos(self.cursor_x, self.cursor_y);
                                }
                                Err(_) => break,
                            }
                        }
                        self.window.request_redraw();
                    }
                    _ => {}
                }
            })
            .expect("Event loop failed");
    }
}
