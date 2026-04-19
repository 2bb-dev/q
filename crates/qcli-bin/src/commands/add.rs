use std::io::Read;

use anyhow::Result;
use qcli_core::Prompt;

use super::{open_queue, save_queue};

pub fn run(text: Option<String>, pin: bool) -> Result<()> {
    let text = match text {
        Some(t) => t,
        None => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            buf
        }
    };
    let (mut queue, _lock, path) = open_queue()?;
    let mut prompt = Prompt::new(text)?;
    prompt.pinned = pin;
    let id = queue.add(prompt);
    save_queue(&path, &queue)?;
    println!("added {id}");
    Ok(())
}
