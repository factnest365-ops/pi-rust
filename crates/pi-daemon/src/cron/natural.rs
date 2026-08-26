use anyhow::{Result, anyhow};
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NaturalUnit {
    Minutes,
    Hours,
    Days,
    Weeks,
}

impl NaturalUnit {
    fn to_duration(self, amount: u64) -> Duration {
        match self {
            NaturalUnit::Minutes => Duration::from_secs(amount * 60),
            NaturalUnit::Hours => Duration::from_secs(amount * 60 * 60),
            NaturalUnit::Days => Duration::from_secs(amount * 60 * 60 * 24),
            NaturalUnit::Weeks => Duration::from_secs(amount * 60 * 60 * 24 * 7),
        }
    }
}

impl FromStr for NaturalUnit {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "min" | "mins" | "minute" | "minutes" => Ok(NaturalUnit::Minutes),
            "hr" | "hrs" | "hour" | "hours" => Ok(NaturalUnit::Hours),
            "d" | "day" | "days" => Ok(NaturalUnit::Days),
            "wk" | "wks" | "week" | "weeks" => Ok(NaturalUnit::Weeks),
            _ => Err(anyhow!("unknown time unit: {}", s)),
        }
    }
}

pub fn parse_duration(input: impl AsRef<str>) -> Result<Duration> {
    let text = input.as_ref().trim().to_lowercase();
    let text = text.strip_prefix("every").unwrap_or(&text).trim();

    if text.is_empty() {
        return Err(anyhow!("empty natural language interval"));
    }

    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.is_empty() {
        return Err(anyhow!("empty natural language interval"));
    }

    let amount = parts[0]
        .parse::<u64>()
        .map_err(|_| anyhow!("invalid amount: {}", parts[0]))?;

    if amount == 0 {
        return Err(anyhow!("interval amount must be greater than 0"));
    }

    if parts.len() == 1 {
        return Ok(Duration::from_secs(amount * 60));
    }

    let unit = parts[1].parse::<NaturalUnit>()?;
    Ok(unit.to_duration(amount))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minutes() {
        assert_eq!(
            parse_duration("every 15 mins").unwrap(),
            Duration::from_secs(15 * 60)
        );
    }

    #[test]
    fn parses_hours() {
        assert_eq!(
            parse_duration("every 2 hours").unwrap(),
            Duration::from_secs(2 * 60 * 60)
        );
    }

    #[test]
    fn parses_days() {
        assert_eq!(
            parse_duration("every 1 day").unwrap(),
            Duration::from_secs(24 * 60 * 60)
        );
    }

    #[test]
    fn rejects_zero() {
        assert!(parse_duration("every 0 mins").is_err());
    }

    #[test]
    fn rejects_unknown_unit() {
        assert!(parse_duration("every 2 moons").is_err());
    }
}
