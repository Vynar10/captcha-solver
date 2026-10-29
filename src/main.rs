mod logic;

use logic::{IMG_MASK_DIR, dcode, pick_img};
use reqwest::Client;
use serde_json::json;

pub const URL: &str = "http://localhost:8081/api/challenge";
pub const ANSWER_URL: &str = "http://localhost:8081/api/answer";

// how many challenges to run; raise it when tuning the cutoffs
pub const ROUNDS: usize = 10;

// fetches one challenge, answers it, and returns whether we were right
async fn solve_one(client: &Client) -> Result<bool, Box<dyn std::error::Error>> {
    let v: serde_json::Value = client.get(URL).send().await?.json().await?;

    let id = v["id"].as_str().ok_or("missing id")?;
    let prompt = v["prompt"].as_str().ok_or("missing prompt")?;
    println!("prompt: {prompt}");

    if let Some(images) = v["images"].as_array() {
        for (i, img) in images.iter().enumerate() {
            let b64 = img.as_str().unwrap_or("");

            dcode(b64, i);
        }
    }

    let shapes = pick_img()?;

    let mut selected: Vec<usize> = Vec::new();

    for (index, shape) in shapes {
        if shape == prompt {
            selected.push(index);
        }
    }

    let reply: serde_json::Value = client
        .post(ANSWER_URL)
        .json(&json!({ "id": id, "selected": selected }))
        .send()
        .await?
        .json()
        .await?;

    let correct = reply["correct"].as_bool().unwrap_or(false);
    println!(
        "selected {:?}, expected {}, correct: {correct}\n",
        selected, reply["expected"]
    );

    Ok(correct)
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // creates src/imgs too, since masks lives inside it
    std::fs::create_dir_all(IMG_MASK_DIR)?;

    let client = Client::new();
    let mut wins = 0;

    for _ in 0..ROUNDS {
        if solve_one(&client).await? {
            wins += 1;
        }
    }

    println!("solved {wins}/{ROUNDS}");

    Ok(())
}
