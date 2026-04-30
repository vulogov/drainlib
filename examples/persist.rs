//! Demonstrates JSON persistence: train a parser, save its state, reload it,
//! and continue parsing — clusters are fully preserved across the boundary.

use drainlib::{ChangeType, DrainParser, DrainParserBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "/tmp/drain_state.json";

    // ── Phase 1: train and save ───────────────────────────────────────────────

    println!("=== Phase 1: train ===\n");

    let mut parser = DrainParserBuilder::new()
        .depth(4)
        .sim_threshold(0.5)
        // Add an IPv4 mask on top of the default digit / hex masks.
        .add_mask(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}")
        .build()?;

    let training = [
        "user alice logged in from 192.168.1.10",
        "user bob logged in from 10.0.0.5",
        "disk usage at 82 percent on node1",
        "disk usage at 91 percent on node2",
        "connection 0xdeadbeef to replica closed",
    ];

    for line in &training {
        let r = parser.parse(line);
        println!(
            "  [{:?}] #{} size={} | {}",
            r.change_type, r.id, r.size,
            r.template.join(" "),
        );
    }

    parser.save(path)?;
    println!("\n  Saved {} clusters to {path}\n", parser.clusters().len());

    // ── Phase 2: reload and continue ─────────────────────────────────────────

    println!("=== Phase 2: reload + continue ===\n");

    let mut parser2 = DrainParser::load(path)?;

    let new_lines = [
        "user dave logged in from 172.16.0.1",   // merges into login cluster
        "disk usage at 55 percent on node3",     // merges into disk cluster
        "job 1234 completed in 8 seconds",       // brand-new cluster
    ];

    for line in &new_lines {
        let r = parser2.parse(line);
        let marker = match r.change_type {
            ChangeType::New     => "NEW    ",
            ChangeType::Updated => "UPDATED",
            ChangeType::None    => "MATCH  ",
        };
        println!(
            "  [{marker}] #{} size={} | {}",
            r.id, r.size,
            r.template.join(" "),
        );
    }

    println!("\n  Total clusters after reload: {}", parser2.clusters().len());

    // ── Show the JSON structure ───────────────────────────────────────────────

    println!("\n=== Snapshot (compact JSON) ===\n");
    println!("{}", parser2.to_json()?);

    Ok(())
}
