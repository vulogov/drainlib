use drainlib::DrainParser;

fn main() {
    let logs = [
        "2024-01-15 INFO  user alice logged in from 192.168.1.10",
        "2024-01-15 INFO  user bob logged in from 192.168.1.22",
        "2024-01-15 INFO  user carol logged in from 10.0.0.5",
        "2024-01-15 ERROR request to db1 failed after 3 retries",
        "2024-01-15 ERROR request to db2 failed after 3 retries",
        "2024-01-15 WARN  disk usage at 85 percent on node1",
        "2024-01-15 WARN  disk usage at 92 percent on node2",
        "2024-01-15 WARN  disk usage at 78 percent on node3",
        "2024-01-15 INFO  connection 0xdeadbeef closed",
        "2024-01-15 INFO  connection 0xcafebabe closed",
        "2024-01-15 INFO  job 42 completed in 120 seconds",
        "2024-01-15 INFO  job 99 completed in 34 seconds",
    ];

    let mut parser = DrainParser::new(4, 0.5, 100);

    println!("--- Parsing {} log lines ---\n", logs.len());
    for line in &logs {
        let cluster = parser.parse(line);
        println!(
            "  line  : {}\n  cluster #{} (size={}) template: {}\n",
            line,
            cluster.id,
            cluster.size,
            cluster.template.join(" "),
        );
    }

    let clusters = parser.clusters();
    println!("--- {} unique templates discovered ---\n", clusters.len());
    for c in clusters {
        println!("  [{}] size={:3}  {}", c.id, c.size, c.template.join(" "));
    }
}
