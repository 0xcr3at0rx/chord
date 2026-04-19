use anyhow::Result;
use clap::CommandFactory;
use std::sync::Arc;
use crate::cli::{Cli, Commands};
use crate::core::config::Settings;
use crate::storage::index::LibraryIndex;
use crate::player;

pub async fn handle_command(
    command: Commands,
    settings: Arc<Settings>,
    index: Arc<LibraryIndex>,
) -> Result<()> {
    match command {
        Commands::Play => {
            player::run_player(settings, index).await?;
        }
        Commands::Index => {
            println!("\x1b[1;30m[ DISK ] Rebuilding local music index...\x1b[0m");
            index.update_index(&settings.config.library.music_dir).await?;
            println!("\x1b[1;32m[ DONE ] Index generation complete.\x1b[0m");
        }
        Commands::Dedup { delete } => {
            println!("\x1b[1;30m[ DEDUP ] Scanning library for duplicates...\x1b[0m");
            index.update_index(&settings.config.library.music_dir).await?;
            let tracks = index.get_all_tracks().await;
            
            let mut groups: std::collections::HashMap<String, Vec<Arc<crate::core::models::TrackMetadata>>> = std::collections::HashMap::new();
            for t in tracks {
                // Group by normalized artist and title. Strip non-alphanumeric chars for fuzzier match.
                let mut key = format!("{} - {}", t.artist.to_lowercase(), t.title.to_lowercase());
                key.retain(|c| c.is_alphanumeric() || c.is_whitespace());
                groups.entry(key).or_default().push(t);
            }
            
            let mut total_removed = 0;
            let mut total_freed = 0;
            let mut total_hardlinked = 0;
            
            for (key, mut group) in groups {
                if group.len() > 1 {
                    // Sort group to find the "best" version to keep.
                    // Prefer FLAC > highest bitrate/size.
                    group.sort_by(|a, b| {
                        let score_a = if a.file_path.as_deref().unwrap_or("").ends_with(".flac") { 2 } else { 1 };
                        let score_b = if b.file_path.as_deref().unwrap_or("").ends_with(".flac") { 2 } else { 1 };
                        score_b.cmp(&score_a).then(b.file_size.unwrap_or(0).cmp(&a.file_size.unwrap_or(0)))
                    });
                    
                    let best = &group[0];
                    if let Some(best_path_str) = &best.file_path {
                        let best_path = std::path::Path::new(best_path_str);
                        
                        println!("\x1b[1;33m[ DUP ] Found {} versions of: {}\x1b[0m", group.len(), key);
                        println!("   Keeping (Source): {}", best_path_str);
                        
                        for duplicate in group.iter().skip(1) {
                            if let Some(path_str) = &duplicate.file_path {
                                let path = std::path::Path::new(path_str);
                                
                                // Skip if they are already hardlinked to the exact same inode
                                if let (Ok(best_meta), Ok(dup_meta)) = (std::fs::metadata(best_path), std::fs::metadata(path)) {
                                    use std::os::unix::fs::MetadataExt;
                                    if best_meta.ino() == dup_meta.ino() {
                                        continue;
                                    }
                                    total_freed += dup_meta.len();
                                }

                                println!("   Hardlinking: {}", path_str);
                                if delete {
                                    // Remove the duplicate and replace it with a hardlink to the best version
                                    let _ = std::fs::remove_file(path);
                                    if std::fs::hard_link(best_path, path).is_ok() {
                                        total_hardlinked += 1;
                                    } else {
                                        // Fallback if hardlink fails (e.g. across partitions)
                                        let _ = std::fs::remove_file(path);
                                        total_removed += 1;
                                    }
                                    
                                    // Clean up orphaned lyrics
                                    if !best_path.with_extension("lrc").exists() {
                                        let _ = std::fs::remove_file(path.with_extension("lrc"));
                                    }
                                    if !best_path.with_extension("txt").exists() {
                                        let _ = std::fs::remove_file(path.with_extension("txt"));
                                    }
                                } else {
                                    total_hardlinked += 1;
                                }
                            }
                        }
                    }
                }
            }
            
            if total_hardlinked > 0 || total_removed > 0 {
                if delete {
                    println!("\x1b[1;32m[ DONE ] Deduplication complete. Hardlinked: {}, Deleted: {}. Freed {:.2} MB.\x1b[0m", total_hardlinked, total_removed, total_freed as f64 / 1_048_576.0);
                    index.update_index(&settings.config.library.music_dir).await?;
                } else {
                    println!("\x1b[1;30m[ INFO ] Dry-run complete. Found {} duplicates to hardlink. Would free {:.2} MB. Run with --delete to apply.\x1b[0m", total_hardlinked + total_removed, total_freed as f64 / 1_048_576.0);
                }
            } else {
                println!("\x1b[1;32m[ DONE ] No deduplication needed. Files are already optimal.\x1b[0m");
            }
        }
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "syncify", &mut std::io::stdout());
        }
    }
    Ok(())
}
