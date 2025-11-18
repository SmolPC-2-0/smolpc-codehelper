use serde::{Deserialize, Serialize};

/// Categories of test prompts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PromptCategory {
    Short,
    Medium,
    Long,
    FollowUp,
}

impl PromptCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            PromptCategory::Short => "short",
            PromptCategory::Medium => "medium",
            PromptCategory::Long => "long",
            PromptCategory::FollowUp => "follow-up",
        }
    }
}

/// A single test prompt with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestPrompt {
    pub id: String,
    pub category: PromptCategory,
    pub prompt: String,
}

/// Predefined test prompts for short queries
pub const SHORT_PROMPTS: [&str; 3] = [
    "What is a variable in Python?",
    "How do I print in JavaScript?",
    "Explain a for loop briefly",
];

/// Predefined test prompts for medium-length queries
pub const MEDIUM_PROMPTS: [&str; 3] = [
    "Write a bubble sort function in Python with comments",
    "Create a simple calculator program in JavaScript",
    "Explain classes and objects in Python with an example",
];

/// Predefined test prompts for long queries
pub const LONG_PROMPTS: [&str; 3] = [
    "Explain object-oriented programming concepts with detailed examples in Python",
    "Write a complete web scraper in Python with error handling and documentation",
    "Create a detailed guide for beginners on how to use Git and GitHub",
];

/// Predefined follow-up prompts (require context from previous response)
pub const FOLLOW_UP_PROMPTS: [&str; 3] = [
    "Can you explain that more simply?",
    "Can you add more comments to the code?",
    "What are some common mistakes beginners make with this?",
];

/// Generate the complete test suite
pub fn get_test_suite() -> Vec<TestPrompt> {
    let mut suite = Vec::new();

    // Add short prompts
    for (idx, prompt) in SHORT_PROMPTS.iter().enumerate() {
        suite.push(TestPrompt {
            id: format!("short_{}", idx + 1),
            category: PromptCategory::Short,
            prompt: prompt.to_string(),
        });
    }

    // Add medium prompts
    for (idx, prompt) in MEDIUM_PROMPTS.iter().enumerate() {
        suite.push(TestPrompt {
            id: format!("medium_{}", idx + 1),
            category: PromptCategory::Medium,
            prompt: prompt.to_string(),
        });
    }

    // Add long prompts
    for (idx, prompt) in LONG_PROMPTS.iter().enumerate() {
        suite.push(TestPrompt {
            id: format!("long_{}", idx + 1),
            category: PromptCategory::Long,
            prompt: prompt.to_string(),
        });
    }

    // Add follow-up prompts
    for (idx, prompt) in FOLLOW_UP_PROMPTS.iter().enumerate() {
        suite.push(TestPrompt {
            id: format!("followup_{}", idx + 1),
            category: PromptCategory::FollowUp,
            prompt: prompt.to_string(),
        });
    }

    suite
}

/// Get total number of tests (prompts × iterations)
pub fn get_total_test_count(iterations: usize) -> usize {
    let prompts_per_iteration = SHORT_PROMPTS.len() + MEDIUM_PROMPTS.len() + LONG_PROMPTS.len() + FOLLOW_UP_PROMPTS.len();
    prompts_per_iteration * iterations
}
