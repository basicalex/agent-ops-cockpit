pub mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    
    // Simple ANSI for now to ensure compatibility
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE: &str = "\x1b[37m";
    
    // Status colors
    pub const SUCCESS: &str = GREEN;
    pub const ERROR: &str = RED;
    pub const WARNING: &str = YELLOW;
    pub const INFO: &str = BLUE;
}

pub mod icons {
    pub const CHECK: &str = ""; // nf-fa-check_circle
    pub const PENDING: &str = ""; // nf-fa-circle_o
    pub const IN_PROGRESS: &str = ""; // nf-fa-dot_circle_o
    pub const BLOCKED: &str = ""; // nf-fa-times_circle
    pub const UNKNOWN: &str = ""; // nf-fa-question_circle
    pub const AGENT: &str = "󰚩";   // nf-md-robot
    
    pub const PRIORITY_HIGH: &str = "🔥";
    pub const PRIORITY_MED: &str = ""; // nf-fa-angle_double_up
    pub const PRIORITY_LOW: &str = ""; // nf-fa-angle_double_down
}
