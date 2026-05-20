//! [`Faker`] — simple random data generator for factory definitions.

use rand::Rng;
use uuid::Uuid;

/// Simple random data generator for use in factory `definition()` methods.
pub struct Faker;

impl Faker {
    /// Random first name.
    pub fn name() -> String {
        let names = [
            "Alice", "Bob", "Carol", "Dave", "Eve", "Frank", "Grace", "Hank", "Iris", "Jack",
            "Karen", "Leo", "Mia", "Nora", "Oscar", "Pam",
        ];
        let idx = rand::thread_rng().gen_range(0..names.len());
        names[idx].to_string()
    }

    /// Random last name.
    pub fn last_name() -> String {
        let names = [
            "Smith", "Jones", "Williams", "Brown", "Taylor", "Anderson", "Wilson", "Moore",
            "Jackson", "White", "Harris", "Martin",
        ];
        let idx = rand::thread_rng().gen_range(0..names.len());
        names[idx].to_string()
    }

    /// Random full name.
    pub fn full_name() -> String {
        format!("{} {}", Self::name(), Self::last_name())
    }

    /// Random email based on the name.
    pub fn email() -> String {
        format!(
            "{}.{}@example.com",
            Self::name().to_lowercase(),
            Self::last_name().to_lowercase(),
        )
    }

    /// Random UUID string.
    pub fn uuid() -> String {
        Uuid::new_v4().to_string()
    }

    /// Random integer in the given range (inclusive).
    pub fn integer(lo: i64, hi: i64) -> i64 {
        rand::thread_rng().gen_range(lo..=hi)
    }

    /// Random boolean (50/50).
    pub fn boolean() -> bool {
        rand::thread_rng().gen_bool(0.5)
    }

    /// Random lorem ipsum word.
    pub fn word() -> String {
        let words = [
            "lorem",
            "ipsum",
            "dolor",
            "sit",
            "amet",
            "consectetur",
            "adipiscing",
            "elit",
            "sed",
            "eiusmod",
            "tempor",
            "incididunt",
        ];
        let idx = rand::thread_rng().gen_range(0..words.len());
        words[idx].to_string()
    }

    /// Random sentence of `n` words.
    pub fn sentence(words: usize) -> String {
        let mut s: Vec<String> = (0..words).map(|_| Self::word()).collect();
        if let Some(first) = s.first_mut() {
            let mut chars = first.chars();
            *first = chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default();
        }
        s.join(" ") + "."
    }

    /// Random URL.
    pub fn url() -> String {
        format!("https://example.com/{}", Self::uuid())
    }

    /// Random phone number string.
    pub fn phone() -> String {
        let mut rng = rand::thread_rng();
        format!(
            "+1-{:03}-{:03}-{:04}",
            rng.gen_range(200..=999),
            rng.gen_range(200..=999),
            rng.gen_range(1000..=9999),
        )
    }

    /// Random password (plain text — hash it before storing!).
    pub fn password() -> String {
        format!("pass-{}", &Uuid::new_v4().to_string()[..8])
    }
}
