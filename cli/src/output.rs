use colored::Colorize;

pub fn header(title: &str) {
    println!("{}", title.bold().underline());
}

pub fn subheader(title: &str) {
    println!("{}", title.bold());
}

pub fn hint(msg: &str) {
    println!("{} {}", "hint:".cyan().bold(), msg.dimmed());
}

pub fn info(msg: &str) {
    eprintln!("{} {}", "info:".blue().bold(), msg);
}

pub fn warn(msg: &str) {
    eprintln!("{} {}", "warning:".yellow().bold(), msg);
}

#[allow(dead_code)]
pub fn error(msg: &str) {
    eprintln!("{} {}", "error:".red().bold(), msg);
}

#[allow(dead_code)]
pub fn success(msg: &str) {
    println!("{} {}", "✓".green().bold(), msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_does_not_panic() {
        header("Test Header");
    }

    #[test]
    fn test_subheader_does_not_panic() {
        subheader("Test Subheader");
    }

    #[test]
    fn test_hint_does_not_panic() {
        hint("This is a hint");
    }

    #[test]
    fn test_info_does_not_panic() {
        info("This is info");
    }

    #[test]
    fn test_warn_does_not_panic() {
        warn("This is a warning");
    }

    #[test]
    fn test_error_does_not_panic() {
        error("This is an error");
    }

    #[test]
    fn test_success_does_not_panic() {
        success("This is success");
    }
}
