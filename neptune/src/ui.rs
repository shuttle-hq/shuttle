use cargo_shuttle::args::OutputMode;
use crossterm::style::Stylize;

pub struct AiUi {
    is_human: bool,
    verbose: bool,
}

impl AiUi {
    pub fn new(output_mode: &OutputMode, verbose: bool) -> Self {
        let is_human = !matches!(output_mode, OutputMode::Json);
        Self { is_human, verbose }
    }

    fn is_human(&self) -> bool {
        self.is_human
    }

    pub fn header(&self, title: &str) {
        if self.is_human() {
            eprintln!();
            eprintln!(
                "{} {}",
                "🔵 Neptune".blue().bold(),
                format!("• {}", title).bold()
            );
        }
    }

    pub fn step<M: AsRef<str>>(&self, emoji: &str, message: M) {
        if self.is_human() {
            eprintln!("  {} {}", emoji, message.as_ref());
        }
    }

    pub fn info<M: AsRef<str>>(&self, message: M) {
        if self.is_human() {
            eprintln!("   ℹ️  {}", message.as_ref());
        }
    }

    pub fn success<M: AsRef<str>>(&self, message: M) {
        if self.is_human() {
            eprintln!("   {}", message.as_ref().green());
        }
    }

    pub fn warn<M: AsRef<str>>(&self, message: M) {
        if self.is_human() {
            eprintln!("   ⚠️  {}", message.as_ref().yellow());
        }
    }

    pub fn done(&self) {
        if self.is_human() {
            eprintln!("  🎉 All set");
        }
    }

    pub fn verbose<M: AsRef<str>>(&self, emoji: &str, message: M) {
        if self.is_human() && self.verbose {
            eprintln!("  {} {}", emoji, message.as_ref());
        }
    }
}
