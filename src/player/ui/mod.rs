use crate::player::app::{App, InputMode};
use crate::player::ui::components::{format_duration, render_visualizer};
use crate::player::ui::theme::THEME;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};
use ratatui_image::{Resize, StatefulImage};

pub mod components;
pub mod theme;

pub fn ui(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Background
    f.render_widget(Block::default().style(Style::default().bg(THEME.bg)), size);

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(size);

    // 0. TOP BAR
    let playlist_name = app
        .current_playlist
        .as_ref()
        .map(|p| p.name.as_str())
        .unwrap_or("All ( Library )");

    let top_bar_spans = vec![
        Span::styled(
            " CHORD ",
            Style::default()
                .fg(THEME.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(
            format!(" {} ", playlist_name.to_uppercase()),
            Style::default().fg(THEME.dim),
        ),
    ];

    // Add audio device to top right
    let device_name = app
        .audio
        .device_name
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| "NONE".to_string());
    let audio_span = Span::styled(
        format!(" [ AUDIO: {} ] ", device_name.to_uppercase()),
        Style::default()
            .fg(THEME.accent_dim)
            .add_modifier(Modifier::BOLD),
    );

    let top_bar_para =
        Paragraph::new(Line::from(top_bar_spans)).style(Style::default().bg(THEME.status_bg));
    f.render_widget(top_bar_para, main_layout[0]);

    f.render_widget(
        Paragraph::new(Line::from(vec![audio_span])).alignment(Alignment::Right),
        main_layout[0],
    );

    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(main_layout[1]);

    // 1. SIDEBAR & 2. MAIN AREA
    {
        if app.input_mode == InputMode::PlaylistSelect {
            let mut items = vec![ListItem::new(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("All ( Library )", Style::default()),
            ]))];

            for p in &app.playlists {
                items.push(ListItem::new(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(&p.name, Style::default()),
                ])));
            }

            let sidebar = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::RIGHT)
                        .border_style(Style::default().fg(THEME.status_bg)),
                )
                .highlight_style(Style::default().bg(THEME.cursor_bg).fg(THEME.cursor_fg));
            f.render_stateful_widget(sidebar, content_layout[0], &mut app.playlist_list_state);
        } else if app.filtered_tracks.is_empty() {
            let empty_msg = vec![
                Line::from(""),
                Line::from(Span::styled("  LIBRARY IS EMPTY  ", Style::default().fg(THEME.critical).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Music Directory: ", Style::default().fg(THEME.dim)),
                    Span::styled(format!("{}", app.settings.config.library.music_dir.display()), Style::default().fg(THEME.accent_dim)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::raw("  [ "),
                    Span::styled("r", Style::default().fg(THEME.accent)),
                    Span::raw(" ] Scan for music"),
                ]),
                Line::from(vec![
                    Span::raw("  [ "),
                    Span::styled("/", Style::default().fg(THEME.accent)),
                    Span::raw(" ] Filter (once music is found)"),
                ]),
                Line::from(""),
                Line::from(Span::styled("  Add music files (FLAC, MP3, etc.) to the", Style::default().fg(THEME.dim))),
                Line::from(Span::styled("  folder above and press 'r' to refresh.", Style::default().fg(THEME.dim))),
            ];
            
            let sidebar = Paragraph::new(empty_msg)
                .block(
                    Block::default()
                        .borders(Borders::RIGHT)
                        .border_style(Style::default().fg(THEME.status_bg)),
                );
            f.render_widget(sidebar, content_layout[0]);
        } else {
            let items: Vec<ListItem> = app
                .filtered_tracks
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let is_playing = app.playing_idx == Some(i);
                    let style = if is_playing {
                        Style::default()
                            .fg(THEME.accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(THEME.fg)
                    };

                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(if is_playing { "> " } else { "  " }, style),
                            Span::styled(&t.title, style),
                        ]),
                        Line::from(vec![
                            Span::raw("    "),
                            Span::styled(&t.artist, Style::default().fg(THEME.dim)),
                        ]),
                    ])
                })
                .collect();

            let sidebar = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::RIGHT)
                        .border_style(Style::default().fg(THEME.status_bg)),
                )
                .highlight_style(Style::default().bg(THEME.cursor_bg).fg(THEME.cursor_fg));
            f.render_stateful_widget(sidebar, content_layout[0], &mut app.list_state);
        }

        let main_area_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(12), Constraint::Min(0)])
            .split(content_layout[1]);

        if let Some(error) = &app.last_error {
            let err_block = Paragraph::new(format!(" ERROR: {} ", error))
                .style(
                    Style::default()
                        .fg(THEME.bg)
                        .bg(THEME.critical)
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .block(Block::default().padding(Padding::uniform(1)));
            f.render_widget(err_block, main_area_layout[0]);
        } else if let Some(track) = &app.current_track {
            // Dashboard
            let dashboard_block = Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(THEME.status_bg))
                .padding(Padding::new(2, 2, 1, 1));

            let dashboard_area = dashboard_block.inner(main_area_layout[0]);
            f.render_widget(dashboard_block, main_area_layout[0]);

            let dash_layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(22), // Image Area (slightly wider for better fit)
                    Constraint::Min(0),     // Info Area
                ])
                .split(dashboard_area);

            // --- IMAGE PREVIEW ---
            if let Some(img) = &app.cached_image {
                if app.image_state.is_none() {
                    let mut picker = app
                        .image_picker
                        .unwrap_or_else(|| ratatui_image::picker::Picker::from_fontsize((7, 14)));

                    app.image_state = Some(picker.new_resize_protocol(img.clone()));
                }

                if let Some(state) = &mut app.image_state {
                    let image = StatefulImage::new(None).resize(Resize::Fit(None));
                    f.render_stateful_widget(image, dash_layout[0], state);
                }
            } else {
                let placeholder = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(THEME.status_bg));
                f.render_widget(placeholder, dash_layout[0]);
            }

            let dash_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // Title
                    Constraint::Length(1), // Artist
                    Constraint::Length(1), // Album
                    Constraint::Length(3), // Visualizer (Fixed height for single row)
                    Constraint::Min(0),    // Tech Footer (Moves up)
                ])
                .split(dash_layout[1]);

            // --- ROW 1: Title ---
            f.render_widget(
                Paragraph::new(Line::from(vec![Span::styled(
                    &track.title,
                    Style::default()
                        .fg(THEME.accent)
                        .add_modifier(Modifier::BOLD),
                )]))
                .alignment(Alignment::Left),
                dash_chunks[0].inner(Margin::new(2, 0)),
            );

            // --- ROW 2: Artist ---
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("Artist: ", Style::default().fg(THEME.dim)),
                    Span::styled(&track.artist, Style::default().fg(THEME.fg)),
                ]))
                .alignment(Alignment::Left),
                dash_chunks[1].inner(Margin::new(2, 0)),
            );

            // --- ROW 3: Album ---
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("Album:  ", Style::default().fg(THEME.dim)),
                    Span::styled(
                        track.album.as_deref().unwrap_or("Unknown"),
                        Style::default().fg(THEME.fg).add_modifier(Modifier::ITALIC),
                    ),
                ]))
                .alignment(Alignment::Left),
                dash_chunks[2].inner(Margin::new(2, 0)),
            );

            // --- ROW 4: Visualizer ---
            let vis_area = dash_chunks[3];
            let vis_width = vis_area.width.saturating_sub(4);

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            let vis_line = render_visualizer(app.is_playing, vis_width, now, app.volume as f64);
            f.render_widget(
                Paragraph::new(vec![Line::from(""), Line::from(vis_line), Line::from("")])
                    .alignment(Alignment::Center),
                vis_area,
            );

            // --- ROW 5: Tech Footer ---
            // Determine track index and total based on the playlist/context from which playback started
            let (track_idx, total_count) = {
                let idx = app.playback_track_list.iter().position(|t| t.track_id == track.track_id);
                (idx, app.playback_track_list.len())
            };

            let mut tech_spans = vec![
                Span::styled(
                    format!(
                        " {:02} / {:02} ",
                        track_idx.map(|i| i + 1).unwrap_or(0),
                        total_count
                    ),
                    Style::default()
                        .fg(THEME.bg)
                        .bg(THEME.accent_dim)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
            ];

            if app.sample_rate > 0 {
                tech_spans.push(Span::styled(" • ", Style::default().fg(THEME.status_bg)));
                tech_spans.push(Span::styled(
                    format!("{}Hz", app.sample_rate),
                    Style::default().fg(THEME.dim),
                ));
            }

            if app.bit_depth > 0 {
                tech_spans.push(Span::styled(" • ", Style::default().fg(THEME.status_bg)));
                tech_spans.push(Span::styled(
                    format!("{}bit", app.bit_depth),
                    Style::default().fg(THEME.dim),
                ));
            }

            if app.bitrate > 0 {
                tech_spans.push(Span::styled(" • ", Style::default().fg(THEME.status_bg)));
                tech_spans.push(Span::styled(
                    format!("{}kbps", app.bitrate),
                    Style::default().fg(THEME.dim),
                ));
            }

            if app.channels > 0 {
                tech_spans.push(Span::styled(" • ", Style::default().fg(THEME.status_bg)));
                tech_spans.push(Span::styled(
                    format!("{}ch", app.channels),
                    Style::default().fg(THEME.dim),
                ));
            }

            f.render_widget(
                Paragraph::new(Line::from(tech_spans)).alignment(Alignment::Right),
                dash_chunks[4].inner(Margin::new(2, 0)),
            );
        } else {
            let empty_block = Block::default().padding(Padding::uniform(2));
            f.render_widget(empty_block, main_area_layout[0]);
        }

        if app.last_error.is_none() {
            let mut lyrics_lines = Vec::new();
            if app.lyrics.is_empty() || (app.lyrics.len() == 1 && app.lyrics[0].text == "NO LYRICS")
            {
                lyrics_lines.push(Line::from(vec![Span::styled(
                    "*  INSTRUMENTAL  *",
                    Style::default()
                        .fg(THEME.dim)
                        .add_modifier(Modifier::ITALIC),
                )]));
            } else {
                let cur_idx = app.current_lyric_idx;
                for (i, l) in app.lyrics.iter().enumerate() {
                    let distance = (i as i32 - cur_idx as i32).abs();
                    if distance > 8 {
                        continue;
                    }

                    let style = match distance {
                        0 => Style::default()
                            .fg(THEME.accent)
                            .add_modifier(Modifier::BOLD),
                        1 => Style::default().fg(Color::Rgb(160, 160, 160)),
                        2 => Style::default().fg(Color::Rgb(100, 100, 100)),
                        3 => Style::default().fg(Color::Rgb(70, 70, 70)),
                        _ => Style::default().fg(Color::Rgb(40, 40, 40)),
                    };

                    if distance == 0 {
                        lyrics_lines.push(Line::from(""));
                        lyrics_lines.push(Line::from(Span::styled(
                            format!("  {}  ", l.text.to_uppercase()),
                            style,
                        )));
                        lyrics_lines.push(Line::from(""));
                    } else {
                        lyrics_lines.push(Line::from(Span::styled(&l.text, style)));
                    }
                }
            }

            f.render_widget(
                Paragraph::new(lyrics_lines)
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: true })
                    .scroll((app.lyrics_scroll, 0))
                    .block(Block::default().padding(Padding::new(0, 0, 1, 0))),
                main_area_layout[1],
            );
        }
    }

    // 3. HELIX-STYLE BOTTOM BAR / HELP OVERLAY
    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(12),     // Mode
            Constraint::Min(0),         // Search / Title
            Constraint::Percentage(25), // Progress Bar
            Constraint::Length(12),     // Duration
            Constraint::Length(10),     // Vol
        ])
        .split(main_layout[2]);

    let (mode_str, mode_style) = match app.input_mode {
        InputMode::Normal => (
            " NORMAL ",
            Style::default()
                .fg(THEME.bg)
                .bg(THEME.dim)
                .add_modifier(Modifier::BOLD),
        ),
        InputMode::Search => (
            " SEARCH ",
            Style::default()
                .fg(THEME.bg)
                .bg(THEME.accent)
                .add_modifier(Modifier::BOLD),
        ),
        InputMode::PlaylistSelect => (
            " PLAYLIST ",
            Style::default()
                .fg(THEME.bg)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
    };

    f.render_widget(
        Paragraph::new(mode_str)
            .alignment(Alignment::Center)
            .style(mode_style),
        status_chunks[0],
    );

    let mid_text = if app.input_mode == InputMode::Search {
        format!(" / {} ", app.search_query)
    } else if let Some(track) = &app.current_track {
        format!(" Playing: {} - {} ", track.artist, track.title)
    } else {
        " CHORD - LIBRARY ".to_string()
    };
    f.render_widget(
        Paragraph::new(mid_text).style(Style::default().fg(THEME.fg).bg(THEME.status_bg)),
        status_chunks[1],
    );

    // Progress Mini-Gauge
    let progress_bar = Gauge::default()
        .gauge_style(Style::default().fg(THEME.accent).bg(THEME.status_bg))
        .use_unicode(true)
        .ratio(app.progress as f64)
        .label("");
    f.render_widget(progress_bar, status_chunks[2]);

    let duration_str = format!(
        " {}/{} ",
        format_duration(app.current_pos),
        format_duration(app.current_track_duration)
    );
    f.render_widget(
        Paragraph::new(duration_str)
            .alignment(Alignment::Right)
            .style(Style::default().fg(THEME.dim).bg(THEME.status_bg)),
        status_chunks[3],
    );

    f.render_widget(
        Paragraph::new(format!(" VOL {}% ", (app.volume * 100.0) as u32))
            .alignment(Alignment::Right)
            .style(Style::default().fg(THEME.accent_dim).bg(THEME.status_bg)),
        status_chunks[4],
    );
}
