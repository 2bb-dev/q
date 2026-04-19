use anyhow::Result;

use super::open_queue;

pub fn run(json: bool) -> Result<()> {
    let (queue, _lock, _path) = open_queue()?;
    if json {
        let arr: Vec<_> = queue.iter().collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
        return Ok(());
    }
    if queue.is_empty() {
        println!("(queue empty)");
        return Ok(());
    }
    for p in queue.iter() {
        let marker = if p.pinned { "[P]" } else { "   " };
        println!("{marker} {} {}", p.id, p.preview());
    }
    Ok(())
}
