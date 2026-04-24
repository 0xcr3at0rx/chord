use std::time::Duration;

pub fn format_duration(d: Duration) -> String {
    let s = d.as_secs();
    let hours = s / 3600;
    let minutes = (s % 3600) / 60;
    let seconds = s % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        // Zero
        assert_eq!(format_duration(Duration::from_secs(0)), "00:00");
        
        // Seconds only
        assert_eq!(format_duration(Duration::from_secs(5)), "00:05");
        assert_eq!(format_duration(Duration::from_secs(59)), "00:59");
        
        // Minutes
        assert_eq!(format_duration(Duration::from_secs(60)), "01:00");
        assert_eq!(format_duration(Duration::from_secs(61)), "01:01");
        assert_eq!(format_duration(Duration::from_secs(3599)), "59:59");
        
        // Hours
        assert_eq!(format_duration(Duration::from_secs(3600)), "1:00:00");
        assert_eq!(format_duration(Duration::from_secs(3661)), "1:01:01");
        assert_eq!(format_duration(Duration::from_secs(86400)), "24:00:00"); // 1 day
    }
}
