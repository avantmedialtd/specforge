use openspec_core::parse_tasks_md;
use std::path::PathBuf;

fn main() {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: dump_tasks <path-to-tasks.md>");

    let parsed = parse_tasks_md(&path).expect("parse failed");

    println!("total_tasks: {}", parsed.total_tasks);
    println!("completed:   {}", parsed.completed_tasks);
    println!("sections:    {}\n", parsed.sections.len());

    for (i, section) in parsed.sections.iter().enumerate() {
        println!("[{i}] {}", section.title);
        for task in &section.tasks {
            let mark = if task.completed { "x" } else { " " };
            println!("    [{}] {}", mark, task.text);
        }
    }
}
