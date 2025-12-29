use crate::vm::{VM, VMState};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use std::io;

pub fn run_tui(mut vm: VM) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut auto_run = false;
    let mut input_buffer = String::new();

    loop {
        terminal.draw(|f| ui(f, &vm, &input_buffer))?;

        if auto_run && vm.state == VMState::Running {
            vm.step();
            // Add a small delay if needed, or just run fast
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        if event::poll(std::time::Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            if vm.state == VMState::WaitingForInput {
                match key.code {
                    KeyCode::Enter => {
                        if let Ok(val) = input_buffer.trim().parse::<i64>() {
                            vm.input_queue.push(val);
                            vm.state = VMState::Running;
                            input_buffer.clear();
                        }
                    }
                    KeyCode::Char(c) => {
                        if c.is_ascii_digit() || c == '-' {
                            input_buffer.push(c);
                        }
                    }
                    KeyCode::Backspace => {
                        input_buffer.pop();
                    }
                    KeyCode::Esc => {
                        break;
                    }
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('n') | KeyCode::Char(' ') => {
                        if vm.state == VMState::Running {
                            vm.step();
                        }
                    }
                    KeyCode::Char('r') => {
                        auto_run = !auto_run;
                    }
                    _ => {}
                }
            }
        }

        if vm.state == VMState::Halted && auto_run {
            auto_run = false;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui(f: &mut Frame, vm: &VM, input_buffer: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.area());

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[0]);

    // Code View
    // Ensure we scroll to keep PC visible
    let instructions: Vec<ListItem> = vm
        .code
        .iter()
        .enumerate()
        .map(|(i, instr)| {
            let content = format!("{:3}: {:?} {}, {}", i, instr.f, instr.l, instr.a);
            let style = if i == vm.p {
                Style::default().fg(Color::Black).bg(Color::White)
            } else {
                Style::default()
            };
            ListItem::new(content).style(style)
        })
        .collect();

    let mut code_state = ratatui::widgets::ListState::default();
    code_state.select(Some(vm.p));

    let code_list = List::new(instructions)
        .block(Block::default().borders(Borders::ALL).title("Code"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_stateful_widget(code_list, top_chunks[0], &mut code_state);

    // Stack View
    let mut stack_items = Vec::new();
    for i in 0..vm.t {
        let val = vm.stack[i];
        let mut content = format!("{:3}: {}", i, val);

        if i == vm.b {
            content.push_str(" <--- B");
        }

        if i == vm.b {
            content.push_str(" [SL]");
        }
        if i == vm.b + 1 {
            content.push_str(" [DL]");
        }
        if i == vm.b + 2 {
            content.push_str(" [RA]");
        }

        stack_items.push(ListItem::new(content));
    }

    let stack_list =
        List::new(stack_items).block(Block::default().borders(Borders::ALL).title("Stack"));
    f.render_widget(stack_list, top_chunks[1]);

    // Bottom View (Output & Status)
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    // Output
    let output_text: String = vm.output.join("\n");
    let output_widget = Paragraph::new(output_text)
        .block(Block::default().borders(Borders::ALL).title("Output"))
        .wrap(Wrap { trim: true });
    f.render_widget(output_widget, bottom_chunks[0]);

    // Status
    let status_text = format!(
        "State: {:?}\nP: {}\nB: {}\nT: {}\nI: {:?} {}, {}\n\nControls:\n'n'/'Space': Step\n'r': Run/Pause\n'q': Quit\n\nInput: {}",
        vm.state, vm.p, vm.b, vm.t, vm.i.f, vm.i.l, vm.i.a, input_buffer
    );
    let status_widget =
        Paragraph::new(status_text).block(Block::default().borders(Borders::ALL).title("Status"));
    f.render_widget(status_widget, bottom_chunks[1]);

    // Input Popup if waiting
    if vm.state == VMState::WaitingForInput {
        let area = centered_rect(60, 20, f.area());
        let input_widget = Paragraph::new(format!("Enter number: {}", input_buffer))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Input Required"),
            )
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(Clear, area); // Clear background
        f.render_widget(input_widget, area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
