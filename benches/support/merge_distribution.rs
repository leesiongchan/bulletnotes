use bulletnotes::sync::{
    merge_executor::MergeExecutor,
    text_merge::{MergeDisposition, merge_text},
};
use std::time::Instant;
use unicode_segmentation::UnicodeSegmentation;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        return Err("merge distributions require release mode".into());
    }
    let runtime = tokio::runtime::Runtime::new()?;
    let executor = MergeExecutor::default();
    let mut cases = Vec::new();
    for count in [128, 512, 1024, 2048] {
        let vocabulary = ["fox", "tree", "river", "cloud", "stone", "leaf", "rain"];
        let repeated = (0..count)
            .map(|i| vocabulary[i % vocabulary.len()])
            .collect::<Vec<_>>()
            .join(" ");
        let base = format!("red {repeated} jumps\n");
        cases.push((
            format!("independent-{count}"),
            base.clone(),
            format!("blue {repeated} jumps\n"),
            format!("red {repeated} runs\n"),
            format!("blue {repeated} runs\n"),
            MergeDisposition::Clean,
        ));
        cases.push((
            format!("overlap-{count}"),
            base,
            format!("blue {repeated} leaps\n"),
            format!("green {repeated} jumps\n"),
            format!("green {repeated} leaps\n"),
            MergeDisposition::PreservedOverlap,
        ));
    }
    for bytes in [1_048_576, 1_048_577] {
        cases.push((
            format!("bytes-{bytes}"),
            "a".repeat(bytes),
            "b".repeat(bytes),
            "c".repeat(bytes),
            "c".repeat(bytes),
            if bytes == 1_048_576 {
                MergeDisposition::PreservedOverlap
            } else {
                MergeDisposition::BudgetFallback
            },
        ));
    }
    let repeated = "word ".repeat(3330);
    cases.push((
        "aggregate-token-near-limit".into(),
        format!("red {repeated}jumps"),
        format!("blue {repeated}jumps"),
        format!("red {repeated}runs"),
        format!("blue {repeated}runs"),
        MergeDisposition::Clean,
    ));
    let tokens = "word ".repeat(3400);
    cases.push((
        "aggregate-token-limit".into(),
        tokens.clone(),
        format!("local {tokens}"),
        format!("remote {tokens}"),
        format!("remote {tokens}"),
        MergeDisposition::BudgetFallback,
    ));
    let mut reports = Vec::new();
    for (name, base, local, remote, expected, disposition) in cases {
        let check = merge_text(&base, &local, &remote);
        assert_eq!(check.body, expected, "{name}");
        assert_eq!(check.disposition, disposition, "{name}");
        assert!(check.body.len() <= base.len() + local.len() + remote.len());
        let mut measurements = Vec::new();
        let mut caps = 0;
        for sample in 0..23 {
            // Owned copies are prepared before timing, matching executor admission input.
            let (b, l, r) = (base.clone(), local.clone(), remote.clone());
            let start = Instant::now();
            let result = runtime.block_on(executor.merge(b, l, r));
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(result.body, expected, "{name}");
            assert_eq!(result.disposition, disposition, "{name}");
            if sample >= 3 {
                measurements.push(elapsed);
                caps += usize::from(result.disposition == MergeDisposition::BudgetFallback);
            }
        }
        let mut sorted = measurements.clone();
        sorted.sort_by(f64::total_cmp);
        reports.push(serde_json::json!({"case":name,"samples_ms":measurements,"p50_ms":sorted[9],"p95_ms":sorted[18],"max_ms":sorted[19],"cap_hits":caps,"disposition":format!("{disposition:?}"),"input_bytes":[base.len(),local.len(),remote.len()],"output_bytes":expected.len(),"aggregate_line_tokens":([&base,&local,&remote].iter().map(|s|s.split_inclusive('\n').count()).sum::<usize>()),"aggregate_word_tokens":([&base,&local,&remote].iter().map(|s|s.split_word_bounds().count()).sum::<usize>())}));
    }
    let report = serde_json::json!({"schema":1,"profile":"release","rust":metadata("rustup", &["run","1.99.1","rustc","--version"]),"os":metadata("sw_vers", &["-productVersion"]),"hardware":metadata("sysctl", &["-n","hw.model"]),"cpu":metadata("sysctl", &["-n","machdep.cpu.brand_string"]),"memory_bytes":metadata("sysctl", &["-n","hw.memsize"]),"warmup":3,"samples":20,"entrypoint":"MergeExecutor::merge; merge_text exact assertions outside timing","limits":{"changed_input_bytes_each":1048576,"aggregate_tokens_per_pass":20000,"worker_slots":1,"wait_deadline_ms":150,"merged_output_bound":"base bytes + local bytes + remote bytes"},"cases":reports});
    std::fs::create_dir_all("target/interaction")?;
    std::fs::write(
        "target/interaction/merge-distribution.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "merge_distribution_cases: {}",
        report["cases"].as_array().unwrap().len()
    );
    Ok(())
}

fn metadata(program: &str, args: &[&str]) -> String {
    std::process::Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|result| result.status.success())
        .map(|result| String::from_utf8_lossy(&result.stdout).trim().to_owned())
        .unwrap_or_else(|| "unavailable".into())
}
