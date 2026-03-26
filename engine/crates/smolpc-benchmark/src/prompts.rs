use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptTier {
    Short,
    Medium,
    Long,
}

impl PromptTier {
    pub fn max_tokens(self) -> usize {
        match self {
            Self::Short => 128,
            Self::Medium => 256,
            Self::Long => 512,
        }
    }
}

pub struct BenchmarkPrompt {
    pub id: &'static str,
    pub tier: PromptTier,
    pub content: &'static str,
}

/// Fixed corpus of 10 coding prompts across 3 tiers.
/// All use temperature=0.0 for reproducibility.
pub const PROMPTS: &[BenchmarkPrompt] = &[
    // --- Short tier (4): ~20 token inputs, max_tokens=128 ---
    BenchmarkPrompt {
        id: "short_1",
        tier: PromptTier::Short,
        content: "Write a Python function that checks if a string is a palindrome.",
    },
    BenchmarkPrompt {
        id: "short_2",
        tier: PromptTier::Short,
        content: "What does the map function do in JavaScript? Give a short example.",
    },
    BenchmarkPrompt {
        id: "short_3",
        tier: PromptTier::Short,
        content: "Fix this Python code: for i in range(10) print(i)",
    },
    BenchmarkPrompt {
        id: "short_4",
        tier: PromptTier::Short,
        content: "Write a CSS rule that centers a div horizontally and vertically.",
    },
    // --- Medium tier (4): ~150 token inputs, max_tokens=256 ---
    BenchmarkPrompt {
        id: "medium_1",
        tier: PromptTier::Medium,
        content: "Explain how bubble sort works step by step, then write it in Python with comments explaining each part of the algorithm.",
    },
    BenchmarkPrompt {
        id: "medium_2",
        tier: PromptTier::Medium,
        content: "Write a JavaScript function that takes an array of numbers and returns an object with the mean, median, and mode. Include error handling for empty arrays.",
    },
    BenchmarkPrompt {
        id: "medium_3",
        tier: PromptTier::Medium,
        content: "I'm building a todo list app. Write the HTML structure and JavaScript for adding items to the list, marking them as complete with a strikethrough, and deleting them with a remove button.",
    },
    BenchmarkPrompt {
        id: "medium_4",
        tier: PromptTier::Medium,
        content: "Explain the difference between let, const, and var in JavaScript. When should I use each one? Give examples showing scoping behavior and hoisting for each.",
    },
    // --- Long tier (2): ~500 token inputs, max_tokens=512 ---
    BenchmarkPrompt {
        id: "long_1",
        tier: PromptTier::Long,
        content: "Review this Python code and suggest improvements:\n\n```python\nimport json\n\ndef process_students(filename):\n    f = open(filename)\n    data = json.load(f)\n    results = []\n    for student in data:\n        total = 0\n        for grade in student['grades']:\n            total = total + grade\n        avg = total / len(student['grades'])\n        if avg >= 90:\n            letter = 'A'\n        elif avg >= 80:\n            letter = 'B'\n        elif avg >= 70:\n            letter = 'C'\n        elif avg >= 60:\n            letter = 'D'\n        else:\n            letter = 'F'\n        results.append({'name': student['name'], 'average': avg, 'letter': letter})\n    f.close()\n    return results\n\nstudents = process_students('students.json')\nfor s in students:\n    print(s['name'] + ' got ' + s['letter'])\n```\n\nExplain what problems exist, suggest fixes, and rewrite the improved version.",
    },
    BenchmarkPrompt {
        id: "long_2",
        tier: PromptTier::Long,
        content: "Design a simple REST API for a school library system. The system needs to track books (title, author, ISBN, available copies), students (name, student ID, year group), and loans (which student has which book, due date). For each endpoint, specify the HTTP method, URL path, request body if any, and response format. Include endpoints for: listing all books, searching books by title or author, checking out a book to a student, returning a book, viewing a student's current loans, and listing overdue books. Use JSON for request and response bodies.",
    },
];
