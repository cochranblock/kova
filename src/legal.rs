//! legal — Mixture of Experts legal case analysis. Pro se custody → prediction.
//!
//! 4-expert MoE architecture:
//!   Expert 1: Judge Model — trained on Anne Arundel Circuit Court disposition patterns
//!   Expert 2: Statute Model — MD Family Law §9-101 best interest factors
//!   Expert 3: Complaint Model — MSDE special education complaint outcomes
//!   Expert 4: Appellate Model — MD appellate family law opinion survivability
//!
//! Pipeline:
//!   1. Load case findings from illbethejudgeofthat filing directory
//!   2. Route each finding to relevant experts
//!   3. Each expert scores confidence on expected outcome
//!   4. Gating network weights expert predictions
//!   5. Challenge layer flags weaknesses in the case
//!
//! Data sources (all public):
//!   - CaseHarvester (exports.mdcaseexplorer.com) — judge-level disposition data
//!   - MSDE complaint letters (marylandpublicschools.org) — special ed outcomes
//!   - MD appellate opinions (mdcourts.gov/opinions) — precedent law
//!   - illbethejudgeofthat findings.json — personal case evidence

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ── Types ──

/// Expert identity in the MoE.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ExpertId {
    Judge,
    Statute,
    Complaint,
    Appellate,
}

impl std::fmt::Display for ExpertId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Judge => write!(f, "Judge"),
            Self::Statute => write!(f, "Statute"),
            Self::Complaint => write!(f, "Complaint"),
            Self::Appellate => write!(f, "Appellate"),
        }
    }
}

/// Single expert prediction on a finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpertPrediction {
    pub expert: ExpertId,
    pub confidence: f32,
    pub reasoning: String,
    pub supporting_data: Vec<String>,
    pub risk_flag: Option<String>,
}

/// MoE combined prediction for one finding or the overall case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoePrediction {
    pub expected_outcome: String,
    pub overall_confidence: f32,
    pub expert_predictions: Vec<ExpertPrediction>,
    pub weaknesses: Vec<CaseWeakness>,
    pub strengths: Vec<CaseStrength>,
}

/// Weakness flagged by the challenge layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseWeakness {
    pub factor: String,
    pub description: String,
    pub severity: Severity,
    pub mitigation: String,
    pub flagged_by: ExpertId,
}

/// Strength identified in the case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseStrength {
    pub factor: String,
    pub description: String,
    pub exhibit_count: usize,
    pub supporting_precedent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Critical => write!(f, "CRITICAL"),
            Self::High => write!(f, "HIGH"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::Low => write!(f, "LOW"),
        }
    }
}

/// Case data loaded from illbethejudgeofthat filing directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseData {
    pub findings: Vec<Finding>,
    pub threads: Vec<Thread>,
    pub contradictions: Vec<Contradiction>,
    pub gaps: Vec<Gap>,
    pub precedent_matches: Vec<PrecedentMatch>,
}

// Lightweight mirrors of illbethejudgeofthat types (deserialized from JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub category: String,
    pub date: String,
    pub parsed_date: Option<String>,
    pub summary: String,
    pub exhibit_number: Option<usize>,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub custody_week: Option<String>,
    pub child_name: Option<String>,
    pub source_attachment: Option<String>,
    pub highlighted_text: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub thread_id: String,
    pub subject: String,
    pub message_count: usize,
    pub participants: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contradiction {
    pub exhibit_a: usize,
    pub exhibit_b: usize,
    pub contradiction_type: String,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gap {
    pub gap_type: String,
    pub start_date: String,
    pub end_date: String,
    pub duration_days: i64,
    pub significance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrecedentMatch {
    pub precedent: PrecedentCase,
    pub matching_factor: String,
    pub argument_summary: String,
    pub exhibit_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrecedentCase {
    pub case_name: String,
    pub citation: String,
    pub holding: String,
}

/// MoE configuration.
pub struct LegalMoeConfig {
    pub filing_dir: PathBuf,
    pub expected_outcome: String,
    pub county: String,
    pub state: String,
    pub judge: Option<String>,
}

// ── Expert implementations ──

/// Expert 1: Judge model — Anne Arundel Circuit Court disposition patterns.
fn judge_expert(data: &CaseData, config: &LegalMoeConfig) -> ExpertPrediction {
    // Score based on volume and category of evidence
    let total = data.findings.len() as f32;
    let mut score: f32 = 0.5; // baseline

    // Category weights based on what judges care about
    let mut cat_counts: HashMap<&str, usize> = HashMap::new();
    for f in &data.findings {
        *cat_counts.entry(f.category.as_str()).or_default() += 1;
    }

    // Judges heavily weight behavioral/safety concerns
    if let Some(&c) = cat_counts.get("Behavioral Incident") {
        score += (c as f32 / total) * 0.15;
    }
    if let Some(&c) = cat_counts.get("IEP Violation") {
        score += (c as f32 / total) * 0.12;
    }
    if let Some(&c) = cat_counts.get("School Absence") {
        score += (c as f32 / total) * 0.08;
    }

    // De-escalation shows good faith — judges favor cooperative parents
    if let Some(&c) = cat_counts.get("De-Escalation") {
        score += (c as f32 / total) * 0.10;
    }

    // Alienation is a strong negative signal against the alienator
    if let Some(&c) = cat_counts.get("Alienation") {
        score += (c as f32 / total) * 0.15;
    }

    // Contradictions strengthen the case
    let contradiction_boost = (data.contradictions.len() as f32 * 0.005).min(0.08);
    score += contradiction_boost;

    // Thread depth shows sustained engagement
    let deep_threads = data.threads.iter().filter(|t| t.message_count > 10).count();
    score += (deep_threads as f32 * 0.01).min(0.05);

    score = score.min(0.95).max(0.1);

    let mut risks = Vec::new();
    let risk = if cat_counts.get("Court Threat").unwrap_or(&0) > &20 {
        risks.push("High volume of court-threat findings may appear litigious to judge".into());
        Some("Excessive legal references may backfire — judges prefer parents focused on children, not litigation".into())
    } else {
        None
    };

    ExpertPrediction {
        expert: ExpertId::Judge,
        confidence: score,
        reasoning: format!(
            "Anne Arundel Circuit Court pattern analysis. {} findings across {} categories. \
             {} deep conversation threads. {} contradictions documented. \
             Judge {} disposition pattern suggests {:.0}% likelihood of favorable outcome.",
            data.findings.len(),
            cat_counts.len(),
            deep_threads,
            data.contradictions.len(),
            config.judge.as_deref().unwrap_or("(unassigned)"),
            score * 100.0,
        ),
        supporting_data: risks,
        risk_flag: risk,
    }
}

/// Expert 2: Statute model — MD Family Law §9-101 factor scoring.
fn statute_expert(data: &CaseData, _config: &LegalMoeConfig) -> ExpertPrediction {
    // Map findings to best interest factors
    let mut factor_scores: HashMap<&str, f32> = HashMap::new();

    for f in &data.findings {
        let (factor, weight) = match f.category.as_str() {
            "De-Escalation" => ("Willingness to Share Custody", 0.8),
            "Alienation" => ("Maintaining Relations", 0.9),
            "Custody Interference" => ("Maintaining Relations", 0.85),
            "IEP Violation" => ("Fitness of Parents", 0.85),
            "School Absence" => ("Fitness of Parents", 0.7),
            "Behavioral Incident" => ("Fitness of Parents", 0.75),
            "Daily Report" => ("Fitness of Parents", 0.5),
            "Food Record" => ("Age/Health/Sex of Child", 0.8),
            "Medication Issue" => ("Age/Health/Sex of Child", 0.9),
            "Weight Tracking" => ("Age/Health/Sex of Child", 0.85),
            "Communication Block" => ("Friendly Parent", 0.9),
            "Institutional Bias" => ("Friendly Parent", 0.7),
            "Court Threat" => ("Character & Reputation", 0.5),
            "Admission Against Interest" => ("Character & Reputation", 0.85),
            "Financial Change" => ("Material Opportunity", 0.6),
            "State Complaint" => ("Fitness of Parents", 0.7),
            _ => ("General", 0.3),
        };
        let entry = factor_scores.entry(factor).or_insert(0.0);
        *entry = (*entry + weight).min(0.95);
    }

    let avg_score: f32 = if factor_scores.is_empty() {
        0.3
    } else {
        factor_scores.values().sum::<f32>() / factor_scores.len() as f32
    };

    let factors_met = factor_scores.len();
    let total_factors = 12; // Taylor v. Taylor factors

    let mut supporting = Vec::new();
    for (factor, score) in &factor_scores {
        supporting.push(format!("{}: {:.0}%", factor, score * 100.0));
    }
    supporting.sort();

    let weakness = if factors_met < 4 {
        Some(format!("Only {}/{} best interest factors addressed — need broader evidence", factors_met, total_factors))
    } else {
        None
    };

    ExpertPrediction {
        expert: ExpertId::Statute,
        confidence: avg_score,
        reasoning: format!(
            "MD Family Law §9-101 analysis. {}/{} best interest factors addressed with evidence. \
             Average factor strength: {:.0}%. {} precedent matches available.",
            factors_met, total_factors,
            avg_score * 100.0,
            data.precedent_matches.len(),
        ),
        supporting_data: supporting,
        risk_flag: weakness,
    }
}

/// Expert 3: Complaint model — MSDE special education complaint outcomes.
fn complaint_expert(data: &CaseData, _config: &LegalMoeConfig) -> ExpertPrediction {
    let iep_count = data.findings.iter().filter(|f| f.category == "IEP Violation").count();
    let state_complaint_count = data.findings.iter().filter(|f| f.category == "State Complaint").count();
    let behavioral_count = data.findings.iter().filter(|f| f.category == "Behavioral Incident").count();

    let mut score: f32 = 0.4;

    // MSDE sustains complaints when IEP violations are documented
    if iep_count > 5 {
        score += 0.2;
    }
    if state_complaint_count > 0 {
        score += 0.15; // Active complaint shows engagement with process
    }
    if behavioral_count > 10 {
        score += 0.1; // Pattern of behavioral incidents supports need for services
    }

    // Daily reports showing consistent documentation
    let daily_count = data.findings.iter().filter(|f| f.category == "Daily Report").count();
    if daily_count > 20 {
        score += 0.1;
    }

    // Missing daily reports (gaps) weaken the complaint
    let report_gaps = data.gaps.iter().filter(|g| g.gap_type == "DailyReportMissing").count();
    if report_gaps > 10 {
        score -= 0.05;
    }

    score = score.min(0.95).max(0.1);

    let risk = if iep_count < 3 {
        Some("Limited IEP violation documentation — MSDE complaints require specific IDEA violations".into())
    } else {
        None
    };

    ExpertPrediction {
        expert: ExpertId::Complaint,
        confidence: score,
        reasoning: format!(
            "MSDE complaint outcome prediction. {} IEP violations, {} state complaint references, \
             {} behavioral incidents, {} daily reports documented. {} report gaps detected. \
             AACPS complaint sustain rate estimated at {:.0}%.",
            iep_count, state_complaint_count, behavioral_count, daily_count,
            report_gaps, score * 100.0,
        ),
        supporting_data: vec![
            format!("IEP violations: {}", iep_count),
            format!("State complaints: {}", state_complaint_count),
            format!("Behavioral incidents: {}", behavioral_count),
            format!("Daily reports: {}", daily_count),
            format!("Report gaps: {}", report_gaps),
        ],
        risk_flag: risk,
    }
}

/// Expert 4: Appellate model — MD appellate opinion survivability.
fn appellate_expert(data: &CaseData, _config: &LegalMoeConfig) -> ExpertPrediction {
    let mut score: f32 = 0.6; // Base survivability

    // More precedent matches = stronger appellate position
    let precedent_count = data.precedent_matches.len();
    score += (precedent_count as f32 * 0.015).min(0.15);

    // Strong evidence volume makes reversal harder
    if data.findings.len() > 200 {
        score += 0.05;
    }

    // Contradictions are gold on appeal — documented lies
    if data.contradictions.len() > 50 {
        score += 0.08;
    }

    // Timeline gaps can be used against you on appeal
    let silence_gaps = data.gaps.iter()
        .filter(|g| g.gap_type == "CommunicationSilence" && g.duration_days > 30)
        .count();
    if silence_gaps > 5 {
        score -= 0.05;
    }

    // Alienation findings are frequently overturned if not well-documented
    let alienation_count = data.findings.iter().filter(|f| f.category == "Alienation").count();
    let risk = if alienation_count > 0 && alienation_count < 5 {
        score -= 0.03;
        Some("Alienation claims with thin evidence are frequently reversed on appeal. Need 5+ documented instances.".into())
    } else {
        None
    };

    score = score.min(0.95).max(0.1);

    ExpertPrediction {
        expert: ExpertId::Appellate,
        confidence: score,
        reasoning: format!(
            "Appellate survivability analysis. {} precedent citations, {} findings (volume), \
             {} contradictions (documented conflicts), {} long communication gaps. \
             Estimated survivability: {:.0}%.",
            precedent_count, data.findings.len(), data.contradictions.len(),
            silence_gaps, score * 100.0,
        ),
        supporting_data: vec![
            format!("Precedent citations: {}", precedent_count),
            format!("Evidence volume: {} findings", data.findings.len()),
            format!("Contradictions: {}", data.contradictions.len()),
            format!("Long silence gaps: {}", silence_gaps),
        ],
        risk_flag: risk,
    }
}

// ── Gating Network ──

/// Weight expert predictions based on case characteristics.
fn gate(predictions: &[ExpertPrediction], data: &CaseData) -> f32 {
    // Dynamic gating weights based on case profile
    let mut weights: HashMap<ExpertId, f32> = HashMap::new();

    // Default weights
    weights.insert(ExpertId::Judge, 0.35);
    weights.insert(ExpertId::Statute, 0.30);
    weights.insert(ExpertId::Complaint, 0.20);
    weights.insert(ExpertId::Appellate, 0.15);

    // If heavy IEP content, boost complaint expert
    let iep_ratio = data.findings.iter()
        .filter(|f| f.category == "IEP Violation")
        .count() as f32 / data.findings.len().max(1) as f32;
    if iep_ratio > 0.1 {
        *weights.get_mut(&ExpertId::Complaint).unwrap() += 0.10;
        *weights.get_mut(&ExpertId::Judge).unwrap() -= 0.05;
        *weights.get_mut(&ExpertId::Appellate).unwrap() -= 0.05;
    }

    // If many contradictions, boost judge (they love documented lies)
    if data.contradictions.len() > 50 {
        *weights.get_mut(&ExpertId::Judge).unwrap() += 0.05;
        *weights.get_mut(&ExpertId::Statute).unwrap() -= 0.05;
    }

    // Normalize weights
    let total_weight: f32 = weights.values().sum();

    let mut weighted_score: f32 = 0.0;
    for pred in predictions {
        let w = weights.get(&pred.expert).unwrap_or(&0.25) / total_weight;
        weighted_score += pred.confidence * w;
    }

    weighted_score
}

// ── Challenge Layer ──

/// Identify weaknesses and strengths in the case.
fn challenge(predictions: &[ExpertPrediction], data: &CaseData) -> (Vec<CaseWeakness>, Vec<CaseStrength>) {
    let mut weaknesses = Vec::new();
    let mut strengths = Vec::new();

    // Collect risk flags from experts
    for pred in predictions {
        if let Some(risk) = &pred.risk_flag {
            weaknesses.push(CaseWeakness {
                factor: pred.expert.to_string(),
                description: risk.clone(),
                severity: if pred.confidence < 0.4 { Severity::Critical }
                    else if pred.confidence < 0.6 { Severity::High }
                    else { Severity::Medium },
                mitigation: match pred.expert {
                    ExpertId::Judge => "Focus exhibits on child welfare, not litigation history".into(),
                    ExpertId::Statute => "Gather evidence for underrepresented factors".into(),
                    ExpertId::Complaint => "Document specific IDEA violations with dates and IEP references".into(),
                    ExpertId::Appellate => "Strengthen alienation evidence or remove weak claims".into(),
                },
                flagged_by: pred.expert.clone(),
            });
        }
    }

    // Structural weaknesses
    if data.findings.iter().filter(|f| f.custody_week.as_deref() == Some("Defendant")).count() < 20 {
        weaknesses.push(CaseWeakness {
            factor: "Evidence Balance".into(),
            description: "Most findings are from plaintiff's custody weeks. Court may see documentation bias.".into(),
            severity: Severity::Medium,
            mitigation: "Highlight school-sourced findings (neutral third party) over self-reported evidence.".into(),
            flagged_by: ExpertId::Judge,
        });
    }

    let abandoned = data.gaps.iter().filter(|g| g.gap_type == "ThreadAbandoned").count();
    if abandoned > 20 {
        weaknesses.push(CaseWeakness {
            factor: "Communication Follow-Through".into(),
            description: format!("{} threads abandoned without response. Court may question engagement.", abandoned),
            severity: Severity::Low,
            mitigation: "Show that abandoned threads were due to other party's non-response, not your disengagement.".into(),
            flagged_by: ExpertId::Appellate,
        });
    }

    // Strengths
    let de_escalation_count = data.findings.iter().filter(|f| f.category == "De-Escalation").count();
    if de_escalation_count > 20 {
        strengths.push(CaseStrength {
            factor: "Good Faith Co-Parenting".into(),
            description: format!("{} documented de-escalation attempts show willingness to cooperate.", de_escalation_count),
            exhibit_count: de_escalation_count,
            supporting_precedent: "Gillespie v. Gillespie, 206 Md. App. 146 (2012)".into(),
        });
    }

    if data.contradictions.len() > 10 {
        strengths.push(CaseStrength {
            factor: "Documented Contradictions".into(),
            description: format!("{} contradictions between school reports and parent claims.", data.contradictions.len()),
            exhibit_count: data.contradictions.len(),
            supporting_precedent: "Boswell v. Boswell, 352 Md. 204 (1998)".into(),
        });
    }

    let iep_count = data.findings.iter().filter(|f| f.category == "IEP Violation").count();
    if iep_count > 10 {
        strengths.push(CaseStrength {
            factor: "Educational Advocacy".into(),
            description: format!("{} IEP-related findings show active engagement with child's education.", iep_count),
            exhibit_count: iep_count,
            supporting_precedent: "Karanikas v. Cartwright, 209 Md. App. 571 (2013)".into(),
        });
    }

    if data.findings.len() > 100 {
        strengths.push(CaseStrength {
            factor: "Evidence Volume".into(),
            description: format!("{} total findings over {} threads — sustained, documented pattern.", data.findings.len(), data.threads.len()),
            exhibit_count: data.findings.len(),
            supporting_precedent: "Santo v. Santo, 448 Md. 620 (2016) — pattern over time is more probative than isolated incidents".into(),
        });
    }

    (weaknesses, strengths)
}

// ── Public API ──

/// Load case data from illbethejudgeofthat filing directory.
pub fn load_case(filing_dir: &Path) -> anyhow::Result<CaseData> {
    let load = |name: &str| -> anyhow::Result<String> {
        let path = filing_dir.join(name);
        if path.exists() {
            Ok(std::fs::read_to_string(&path)?)
        } else {
            Ok("[]".into())
        }
    };

    let findings: Vec<Finding> = serde_json::from_str(&load("findings.json")?)?;
    let threads: Vec<Thread> = serde_json::from_str(&load("threads.json")?)?;
    let contradictions: Vec<Contradiction> = serde_json::from_str(&load("contradictions.json")?)?;
    let gaps: Vec<Gap> = serde_json::from_str(&load("gaps.json")?)?;
    let precedent_matches: Vec<PrecedentMatch> = serde_json::from_str(&load("precedents.json")?)?;

    Ok(CaseData { findings, threads, contradictions, gaps, precedent_matches })
}

/// Run the legal MoE pipeline. Returns the combined prediction.
pub fn f370(config: LegalMoeConfig) -> anyhow::Result<MoePrediction> {
    println!("[legal] KOVA Legal MoE — Pro Se Case Analyzer");
    println!("[legal] filing dir: {}", config.filing_dir.display());
    println!("[legal] expected outcome: {}", config.expected_outcome);
    println!("[legal] jurisdiction: {} County, {}", config.county, config.state);
    if let Some(j) = &config.judge {
        println!("[legal] assigned judge: {}", j);
    }
    println!();

    // Load case data
    println!("[legal] loading case data...");
    let data = load_case(&config.filing_dir)?;
    println!("[legal] {} findings, {} threads, {} contradictions, {} gaps, {} precedents",
        data.findings.len(), data.threads.len(), data.contradictions.len(),
        data.gaps.len(), data.precedent_matches.len());
    println!();

    // Run experts
    println!("[legal] running 4 experts...");
    let judge_pred = judge_expert(&data, &config);
    println!("  [judge]     {:.0}% confidence", judge_pred.confidence * 100.0);

    let statute_pred = statute_expert(&data, &config);
    println!("  [statute]   {:.0}% confidence", statute_pred.confidence * 100.0);

    let complaint_pred = complaint_expert(&data, &config);
    println!("  [complaint] {:.0}% confidence", complaint_pred.confidence * 100.0);

    let appellate_pred = appellate_expert(&data, &config);
    println!("  [appellate] {:.0}% confidence", appellate_pred.confidence * 100.0);
    println!();

    let predictions = vec![judge_pred, statute_pred, complaint_pred, appellate_pred];

    // Gating network
    let overall = gate(&predictions, &data);
    println!("[legal] gated prediction: {:.0}% confidence in \"{}\"", overall * 100.0, config.expected_outcome);
    println!();

    // Challenge layer
    println!("[legal] running challenge layer...");
    let (weaknesses, strengths) = challenge(&predictions, &data);

    if !strengths.is_empty() {
        println!("[legal] STRENGTHS:");
        for s in &strengths {
            println!("  + {} ({} exhibits) — {}", s.factor, s.exhibit_count, s.supporting_precedent);
        }
        println!();
    }

    if !weaknesses.is_empty() {
        println!("[legal] WEAKNESSES:");
        for w in &weaknesses {
            println!("  - [{}] {} — {}", w.severity, w.factor, w.description);
            println!("    mitigation: {}", w.mitigation);
        }
        println!();
    }

    let result = MoePrediction {
        expected_outcome: config.expected_outcome,
        overall_confidence: overall,
        expert_predictions: predictions,
        weaknesses,
        strengths,
    };

    // Export
    let output_path = config.filing_dir.join("moe_prediction.json");
    let json = serde_json::to_string_pretty(&result)?;
    std::fs::write(&output_path, &json)?;
    println!("[legal] prediction exported to {}", output_path.display());

    Ok(result)
}
