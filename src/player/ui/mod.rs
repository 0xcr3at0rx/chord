use crate::core::visualizer::render_visualizer;
use crate::player::app::{App, InputMode};
use crate::player::ui::components::format_duration;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};
use ratatui_image::{Resize, StatefulImage};

pub mod components;

pub fn ui(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Background
    f.render_widget(
        Block::default().style(Style::default().bg(app.theme.bg)),
        size,
    );

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
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(
            format!(" {} ", playlist_name.to_uppercase()),
            Style::default().fg(app.theme.dim),
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
            .fg(app.theme.accent_dim)
            .add_modifier(Modifier::BOLD),
    );

    let top_bar_para =
        Paragraph::new(Line::from(top_bar_spans)).style(Style::default().bg(app.theme.status_bg));
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
        let is_search = app.input_mode == InputMode::Search;
        let effective_mode = if is_search {
            app.previous_mode
        } else {
            app.input_mode
        };

        if effective_mode == InputMode::PlaylistSelect {
            let mut items = Vec::new();

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
                        .border_style(Style::default().fg(app.theme.status_bg))
                        .title(Line::from(vec![
                            Span::styled(" SELECT ", Style::default().bg(Color::Magenta).fg(app.theme.bg)),
                            Span::raw(" "),
                            Span::styled("PLAYLIST", Style::default().fg(app.theme.dim)),
                        ])),
                )
                .highlight_style(
                    Style::default()
                        .bg(app.theme.cursor_bg)
                        .fg(app.theme.cursor_fg),
                );
            f.render_stateful_widget(sidebar, content_layout[0], &mut app.playlist_list_state);
        } else if effective_mode == InputMode::Online {
            let mut items = Vec::new();
            for station in &app.filtered_stations {
                let is_playing = app.current_track.as_ref()
                    .map(|t| t.track_id == "radio" && t.title == station.name)
                    .unwrap_or(false);
                
                let style = if is_playing {
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.accent)
                };

                let indicator = if is_playing {
                    if app.is_playing { " 󰐊 " } else { " 󰏤 " }
                } else {
                    "   "
                };

                items.push(ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(indicator, style),
                        Span::styled(format!("{}", station.name), style),
                    ]),
                    Line::from(vec![
                        Span::styled(format!("    {} ", station.country), Style::default().fg(app.theme.dim)),
                    ]),
                    Line::from(""),
                ]));
            }

            let title_style = if is_search {
                Style::default().bg(app.theme.accent).fg(app.theme.bg)
            } else {
                Style::default().bg(Color::Blue).fg(app.theme.bg)
            };

            if items.is_empty() && is_search {
                let msg = format!("  No stations match '{}'", app.search_query);
                let sidebar = Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  NO RESULTS FOUND  ",
                        Style::default()
                            .fg(app.theme.critical)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(msg, Style::default().fg(app.theme.dim))),
                ])
                .block(
                    Block::default()
                        .borders(Borders::RIGHT)
                        .border_style(Style::default().fg(app.theme.status_bg))
                        .title(Line::from(vec![
                            Span::styled(" ONLINE ", title_style),
                            Span::raw(" "),
                            Span::styled("ALL STATIONS", Style::default().fg(app.theme.dim)),
                        ])),
                );
                f.render_widget(sidebar, content_layout[0]);
            } else {
                let sidebar = List::new(items)
                    .block(
                        Block::default()
                            .borders(Borders::RIGHT)
                            .border_style(Style::default().fg(app.theme.status_bg))
                            .title(Line::from(vec![
                                Span::styled(" ONLINE ", title_style),
                                Span::raw(" "),
                                Span::styled("ALL STATIONS", Style::default().fg(app.theme.dim)),
                            ])),
                    )
                    .highlight_style(
                        Style::default()
                            .bg(app.theme.cursor_bg)
                            .fg(app.theme.cursor_fg),
                    );
                f.render_stateful_widget(sidebar, content_layout[0], &mut app.radio_list_state);
            }
        } else if app.filtered_tracks.is_empty() {
            let (title, msg) = if !app.search_query.is_empty() {
                ("  NO RESULTS FOUND  ", format!("  No tracks match '{}'", app.search_query))
            } else {
                ("  LIBRARY IS EMPTY  ", "  Add music files and press 'r' to scan.".to_string())
            };

            let empty_msg = vec![
                Line::from(""),
                Line::from(Span::styled(
                    title,
                    Style::default()
                        .fg(app.theme.critical)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(msg, Style::default().fg(app.theme.dim))),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Music Directory: ", Style::default().fg(app.theme.dim)),
                    Span::styled(
                        format!(
                            "{}",
                            app.settings
                                .config
                                .read()
                                .unwrap()
                                .library
                                .music_dir
                                .display()
                        ),
                        Style::default().fg(app.theme.accent_dim),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::raw("  [ "),
                    Span::styled("r", Style::default().fg(app.theme.accent)),
                    Span::raw(" ] Scan for music"),
                ]),
            ];

            let sidebar = Paragraph::new(empty_msg).block(
                Block::default()
                    .borders(Borders::RIGHT)
                    .border_style(Style::default().fg(app.theme.status_bg)),
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
                            .fg(app.theme.accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(app.theme.fg)
                    };

                    let indicator = if is_playing {
                        if app.is_playing { " 󰐊 " } else { " 󰏤 " }
                    } else {
                        "   "
                    };

                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(indicator, style),
                            Span::styled(&t.title, style),
                        ]),
                        Line::from(vec![
                            Span::raw("    "),
                            Span::styled(&t.artist, Style::default().fg(app.theme.dim)),
                        ]),
                    ])
                })
                .collect();

            let sidebar_title = if let Some(p) = &app.current_playlist {
                format!("PLAYLIST: {}", p.name.to_uppercase())
            } else {
                "LIBRARY".to_string()
            };

            let sidebar = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::RIGHT)
                        .border_style(Style::default().fg(app.theme.status_bg))
                        .title(Line::from(vec![
                            Span::styled(" OFFLINE ", Style::default().bg(app.theme.dim).fg(app.theme.bg)),
                            Span::raw(" "),
                            Span::styled(sidebar_title, Style::default().fg(app.theme.dim)),
                        ])),
                )
                .highlight_style(
                    Style::default()
                        .bg(app.theme.cursor_bg)
                        .fg(app.theme.cursor_fg),
                );
            f.render_stateful_widget(sidebar, content_layout[0], &mut app.list_state);
        }

        let main_area_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(16), Constraint::Min(0)])
            .split(content_layout[1]);

        if let Some(error) = &app.last_error {
            let err_block = Paragraph::new(format!(" ERROR: {} ", error))
                .style(
                    Style::default()
                        .fg(app.theme.bg)
                        .bg(app.theme.critical)
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .block(Block::default().padding(Padding::uniform(1)));
            f.render_widget(err_block, main_area_layout[0]);
        } else if let Some(track) = &app.current_track {
            let is_radio = track.status.as_deref() == Some("radio");
            // Dashboard
            let dashboard_block = Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(app.theme.status_bg))
                .padding(Padding::new(2, 2, 1, 1));

            let dashboard_area = dashboard_block.inner(main_area_layout[0]);
            f.render_widget(dashboard_block, main_area_layout[0]);

            let dash_layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(22), // Image Area
                    Constraint::Min(0),     // Info Area
                ])
                .split(dashboard_area);

            // --- IMAGE / RADIO / CONFIG ICON PREVIEW ---
            if is_radio {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                let real_vol = app.audio.get_amplitude() as f64;
                let art_lines = crate::player::ui::components::render_radio_art(
                    app.is_playing,
                    app.is_starting,
                    20, // width
                    10, // height
                    now,
                    &track.title,
                    &app.theme,
                    real_vol,
                );

                f.render_widget(
                    Paragraph::new(art_lines)
                        .alignment(Alignment::Center)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(app.theme.status_bg)),
                        ),
                    dash_layout[0],
                );
            } else if let Some(img) = &app.cached_image {
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
                    .border_style(Style::default().fg(app.theme.status_bg));
                f.render_widget(placeholder, dash_layout[0]);
            }

            let dash_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // Title
                    Constraint::Length(1), // Artist / Country
                    Constraint::Length(1), // Album / Tags
                    Constraint::Length(1), // GAP 1
                    Constraint::Length(6), // Visualizer
                    Constraint::Length(1), // GAP 2
                    Constraint::Min(0),    // Tech Footer
                ])
                .split(dash_layout[1]);

            // --- ROW 1: Title ---
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    if is_radio {
                        Span::styled(
                            "LIVE: ",
                            Style::default()
                                .fg(app.theme.critical)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        Span::raw("")
                    },
                    Span::styled(
                        &track.title,
                        Style::default()
                            .fg(app.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]))
                .alignment(Alignment::Left),
                dash_chunks[0].inner(Margin::new(2, 0)),
            );

            // --- ROW 2: Artist ---
            let artist_label = if is_radio { "Station: " } else { "Artist:  " };
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(artist_label, Style::default().fg(app.theme.dim)),
                    Span::styled(&track.artist, Style::default().fg(app.theme.fg)),
                ]))
                .alignment(Alignment::Left),
                dash_chunks[1].inner(Margin::new(2, 0)),
            );

            // --- ROW 3: Album ---
            let album_label = if is_radio { "Tags:    " } else { "Album:   " };
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(album_label, Style::default().fg(app.theme.dim)),
                    Span::styled(
                        track.album.as_deref().unwrap_or("Unknown"),
                        Style::default()
                            .fg(app.theme.fg)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]))
                .alignment(Alignment::Left),
                dash_chunks[2].inner(Margin::new(2, 0)),
            );

            // --- ROW 5: Visualizer ---
            let vis_area = dash_chunks[4];
            let vis_width = vis_area.width.saturating_sub(4);
            let vis_height = vis_area.height;

            let visualizer_mode = app.settings.config.read().unwrap().audio.visualizer;
            let real_vol = app.audio.get_amplitude() as f64;
            let vis_lines = render_visualizer(
                app.is_playing,
                vis_width,
                vis_height,
                (app.audio_clock * 100.0) as u64,
                real_vol,
                &app.theme,
                visualizer_mode,
            );
            f.render_widget(
                Paragraph::new(vis_lines).alignment(Alignment::Center),
                vis_area,
            );

            // --- ROW 6: Tech Footer ---
            // Determine track index and total based on the playlist/context from which playback started
            let (track_idx, total_count) = if is_radio {
                (None, 0)
            } else {
                let idx = app
                    .playback_track_list
                    .iter()
                    .position(|t| t.track_id == track.track_id);
                (idx, app.playback_track_list.len())
            };

            let mut tech_spans = Vec::new();

            if !is_radio {
                tech_spans.push(Span::styled(
                    format!(
                        " {:02} / {:02} ",
                        track_idx.map(|i| i + 1).unwrap_or(0),
                        total_count
                    ),
                    Style::default()
                        .fg(app.theme.bg)
                        .bg(app.theme.accent_dim)
                        .add_modifier(Modifier::BOLD),
                ));
                tech_spans.push(Span::raw("  "));

                if app.sample_rate > 0 {
                    tech_spans.push(Span::styled(
                        " * ",
                        Style::default().fg(app.theme.status_bg),
                    ));
                    tech_spans.push(Span::styled(
                        format!("{}Hz", app.sample_rate),
                        Style::default().fg(app.theme.dim),
                    ));
                }

                if app.bit_depth > 0 {
                    tech_spans.push(Span::styled(
                        " * ",
                        Style::default().fg(app.theme.status_bg),
                    ));
                    tech_spans.push(Span::styled(
                        format!("{}bit", app.bit_depth),
                        Style::default().fg(app.theme.dim),
                    ));
                }

                if app.bitrate > 0 {
                    tech_spans.push(Span::styled(
                        " * ",
                        Style::default().fg(app.theme.status_bg),
                    ));
                    tech_spans.push(Span::styled(
                        format!("{}kbps", app.bitrate),
                        Style::default().fg(app.theme.dim),
                    ));
                }

                if app.channels > 0 {
                    tech_spans.push(Span::styled(
                        " * ",
                        Style::default().fg(app.theme.status_bg),
                    ));
                    tech_spans.push(Span::styled(
                        format!("{}ch", app.channels),
                        Style::default().fg(app.theme.dim),
                    ));
                }

                let visualizer_mode_label = app.settings.config.read().unwrap().audio.visualizer;
                tech_spans.push(Span::styled(
                    " * ",
                    Style::default().fg(app.theme.status_bg),
                ));
                tech_spans.push(Span::styled(
                    format!("{:?}", visualizer_mode_label).to_uppercase(),
                    Style::default().fg(app.theme.accent_dim),
                ));
            }

            f.render_widget(
                Paragraph::new(Line::from(tech_spans)).alignment(Alignment::Right),
                dash_chunks[6].inner(Margin::new(2, 0)),
            );
        } else {
            let empty_block = Block::default().padding(Padding::uniform(2));
            f.render_widget(empty_block, main_area_layout[0]);
        }

        if app.last_error.is_none() {
            let mut lyrics_lines = Vec::new();
            let is_radio = app
                .current_track
                .as_ref()
                .map(|t| t.status.as_deref() == Some("radio"))
                .unwrap_or(false);
            if is_radio {
                lyrics_lines.push(Line::from(vec![Span::styled(
                    "--- LIVE BROADCAST ---",
                    Style::default()
                        .fg(app.theme.dim)
                        .add_modifier(Modifier::BOLD),
                )]));
                lyrics_lines.push(Line::from(""));
                lyrics_lines.push(Line::from(vec![Span::styled(
                    "Select a station and press Enter to play",
                    Style::default().fg(app.theme.dim),
                )]));
            } else if app.lyrics.is_empty()
                || (app.lyrics.len() == 1 && app.lyrics[0].text == "NO LYRICS")
            {
                lyrics_lines.push(Line::from(vec![Span::styled(
                    "",
                    Style::default()
                        .fg(app.theme.dim)
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
                            .fg(app.theme.accent)
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
                        lyrics_lines.push(Line::from(Span::styled(l.text.to_uppercase(), style)));
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
    let is_radio = app
        .current_track
        .as_ref()
        .map(|t| t.status.as_deref() == Some("radio"))
        .unwrap_or(false);

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
        InputMode::Offline => (
            " OFFLINE ",
            Style::default()
                .fg(app.theme.bg)
                .bg(app.theme.dim)
                .add_modifier(Modifier::BOLD),
        ),
        InputMode::Search => (
            " SEARCH ",
            Style::default()
                .fg(app.theme.bg)
                .bg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        InputMode::PlaylistSelect => (
            " PLAYLIST ",
            Style::default()
                .fg(app.theme.bg)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        InputMode::Online => (
            " ONLINE ",
            Style::default()
                .fg(app.theme.bg)
                .bg(Color::Blue)
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
    } else if app.input_mode == InputMode::Online {
        " STATIONS ".to_string()
    } else if let Some(track) = &app.current_track {
        format!(" Playing: {} - {} ", track.artist, track.title)
    } else {
        " CHORD - LIBRARY ".to_string()
    };
    f.render_widget(
        Paragraph::new(mid_text).style(Style::default().fg(app.theme.fg).bg(app.theme.status_bg)),
        status_chunks[1],
    );

    // Progress Mini-Gauge
    if !is_radio {
        f.render_widget(
            Paragraph::new(" [ LOCAL PLAYBACK ] ")
                .style(
                    Style::default()
                        .fg(app.theme.accent)
                        .bg(app.theme.status_bg),
                )
                .alignment(Alignment::Center),
            status_chunks[2],
        );
    } else {
        f.render_widget(
            Paragraph::new(" [ LIVE STREAM ] ")
                .style(
                    Style::default()
                        .fg(app.theme.accent)
                        .bg(app.theme.status_bg),
                )
                .alignment(Alignment::Center),
            status_chunks[2],
        );
    }

    let duration_str = if is_radio {
        " --:-- / --:-- ".to_string()
    } else {
        format!(
            " {}/{} ",
            format_duration(app.current_pos),
            format_duration(app.current_track_duration)
        )
    };
    f.render_widget(
        Paragraph::new(duration_str)
            .alignment(Alignment::Right)
            .style(Style::default().fg(app.theme.dim).bg(app.theme.status_bg)),
        status_chunks[3],
    );

    f.render_widget(
        Paragraph::new(format!(" VOL {}% ", (app.volume * 100.0) as u32))
            .alignment(Alignment::Right)
            .style(
                Style::default()
                    .fg(app.theme.accent_dim)
                    .bg(app.theme.status_bg),
            ),
        status_chunks[4],
    );
}
