// SPDX-License-Identifier: Unlicense
// Unlicense — public domain — cochranblock.org
//! exopack capability: ATS fixture generator.
//!
//! Renders HTML strings that mirror the DOM conventions of major
//! Applicant Tracking Systems — Greenhouse, Lever, Workday, iCIMS,
//! Ashby — for use as test bench data. Each vendor-renderer is
//! parameterized by [`FixtureOpts`] so a test can dial in
//! adversarial patterns (late hydration, dynamic IDs,
//! rebuild-on-focus, role=combobox-as-select) without hand-editing
//! HTML files.
//!
//! The generated pages are self-contained — no external CSS/JS, no
//! network — so consumers can `Page::set_content(html)` them via
//! chromiumoxide and exercise CDP-driven autofill end-to-end.
//!
//! Fixtures share a canonical 9-field profile slot inventory:
//!   email, phone, full_name (split first/last), address (street,
//!   city, postal_code), linkedin, github, work_authorization,
//!   freetext-question, plus one out-of-vocab decoy.
//! `expected_keys(vendor)` returns the (id, classifier_key) pairs
//! the consumer should assert against their classifier output.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtsVendor {
    /// Greenhouse Software — `<label for>` + inline forms.
    Greenhouse,
    /// Lever — aria-label-only fields, no visible `<label>`.
    Lever,
    /// Workday — sibling `<span class="wd-label">` carries visible
    /// text; aria-label carries the semantic meaning; dynamic IDs
    /// like `input-1234567890-<uuid>`; late hydration; sometimes a
    /// MutationObserver rebuilds the field on focus.
    Workday,
    /// iCIMS — legacy `<fieldset><legend>` pattern.
    Icims,
    /// Ashby — modern React shapes, `role="combobox"` masquerading
    /// as `<select>`, custom dropdown lists.
    Ashby,
}

impl AtsVendor {
    pub fn label(&self) -> &'static str {
        match self {
            AtsVendor::Greenhouse => "greenhouse",
            AtsVendor::Lever => "lever",
            AtsVendor::Workday => "workday",
            AtsVendor::Icims => "icims",
            AtsVendor::Ashby => "ashby",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FixtureOpts {
    /// Defer form rendering this many milliseconds. Workday-style
    /// late hydration. `None` = render immediately on parse.
    pub late_hydration_ms: Option<u32>,
    /// Generate field IDs of the form `input-<deterministic>-<uuid>`
    /// rather than stable semantic IDs. Workday default behavior.
    /// Deterministic so test snapshots stay reproducible.
    pub dynamic_ids: bool,
    /// On `focus`, destroy and recreate the focused field's parent
    /// container. Mirrors Workday's "rebuild on focus" anti-bot
    /// pattern and forces consumers to handle re-attach.
    pub rebuild_on_focus: bool,
}

/// One field in a fixture's expected schema. Stable across vendors:
/// the same logical field maps to the same `key` regardless of how
/// the vendor renders it.
struct FieldDef {
    /// Short identifier used in IDs. Vendor renderer may decorate.
    slot: &'static str,
    /// What `predict_field_key` should return on this field.
    expected_key: &'static str,
}

const SHARED_FIELDS: &[FieldDef] = &[
    FieldDef { slot: "first",    expected_key: "full_name" },
    FieldDef { slot: "last",     expected_key: "full_name" },
    FieldDef { slot: "email",    expected_key: "email" },
    FieldDef { slot: "phone",    expected_key: "phone" },
    FieldDef { slot: "linkedin", expected_key: "linkedin" },
    FieldDef { slot: "github",   expected_key: "github" },
    FieldDef { slot: "address",  expected_key: "address" },
    FieldDef { slot: "city",     expected_key: "address" },
    FieldDef { slot: "zip",      expected_key: "address" },
    FieldDef { slot: "auth",     expected_key: "work_authorization" },
    FieldDef { slot: "why",      expected_key: "freetext" },
    FieldDef { slot: "salary",   expected_key: "" }, // out-of-vocab decoy
];

/// Compose the deterministic field id the renderer will emit for
/// `slot`, given the opts. With `dynamic_ids=true`, mirrors Workday's
/// `input-<numeric>-<uuid-ish>` pattern.
fn field_id(slot: &str, opts: &FixtureOpts) -> String {
    if opts.dynamic_ids {
        // Stable across runs because we use a deterministic hash, not
        // a real UUID. `input-<seed>-<slot>` keeps the test repeatable.
        format!("input-{}-{slot}", djb2(slot))
    } else {
        slot.to_string()
    }
}

fn djb2(s: &str) -> u32 {
    let mut h: u32 = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u32);
    }
    h
}

/// Public: full HTML string for `vendor` under `opts`.
pub fn render(vendor: AtsVendor, opts: &FixtureOpts) -> String {
    match vendor {
        AtsVendor::Greenhouse => render_greenhouse(opts),
        AtsVendor::Lever => render_lever(opts),
        AtsVendor::Workday => render_workday(opts),
        AtsVendor::Icims => render_icims(opts),
        AtsVendor::Ashby => render_ashby(opts),
    }
}

/// Public: per-vendor (field_id, expected_classifier_key) pairs that
/// the consumer should assert against their classifier output.
pub fn expected_keys(vendor: AtsVendor, opts: &FixtureOpts) -> Vec<(String, &'static str)> {
    SHARED_FIELDS
        .iter()
        .filter(|f| match vendor {
            // Lever uses a single combined `name` field (per
            // jeffistyping/workpls), so the "last" slot doesn't exist
            // there. Address sub-fields also omitted.
            AtsVendor::Lever
                if matches!(f.slot, "last" | "address" | "city" | "zip") =>
            {
                false
            }
            _ => true,
        })
        .map(|f| (field_id(f.slot, opts), f.expected_key))
        .collect()
}

// ─── Renderers ────────────────────────────────────────────────────────────

fn hydration_wrapper(opts: &FixtureOpts, body: &str) -> String {
    let id_dynamic_note = if opts.dynamic_ids { " dynamic-ids" } else { "" };
    let rebuild_js = if opts.rebuild_on_focus {
        r#"
document.addEventListener('focusin', (ev) => {
    const el = ev.target;
    if (!el.matches('input, textarea, select')) return;
    const parent = el.parentElement;
    const html = parent.innerHTML;
    parent.innerHTML = '';
    setTimeout(() => { parent.innerHTML = html; }, 50);
}, true);
"#
    } else {
        ""
    };
    match opts.late_hydration_ms {
        None => format!(
            "<div id=\"app\"{id_dynamic_note}>{body}</div><script>{rebuild_js}</script>"
        ),
        Some(ms) => format!(
            r#"<div id="app"{note}><p>Loading…</p></div>
<script>
setTimeout(() => {{
    document.getElementById('app').innerHTML = {body_json};
    {rebuild}
}}, {ms});
</script>"#,
            note = id_dynamic_note,
            body_json = json_escape_string(body),
            rebuild = rebuild_js,
            ms = ms
        ),
    }
}

fn json_escape_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '<' => out.push_str("\\u003c"),
            '/' => out.push_str("\\/"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn render_greenhouse(opts: &FixtureOpts) -> String {
    // Production selectors observed in jeffistyping/workpls (Jeffrey Hui)
    // and josephajibodu/greenhouse-autofill-chrome-extension. Greenhouse
    // wraps name fields in `job_application[first_name]` brackets, has
    // bare `email`/`phone` keys, and uses `urls[LinkedIn]`/`urls[Github]`
    // with the literal-trailing-space `urls[Github ]` quirk that real
    // companies emit when they add the field through Greenhouse's admin.
    let id = |s: &str| field_id(s, opts);
    let body = format!(
        r#"<form id="application_form"><div class="row">
<div class="field"><label for="{first}">First Name</label><input type="text" id="{first}" name="job_application[first_name]" required></div>
<div class="field"><label for="{last}">Last Name</label><input type="text" id="{last}" name="job_application[last_name]" required></div></div>
<div class="field"><label for="{email}">Email</label><input type="email" id="{email}" name="email" placeholder="you@example.com" required></div>
<div class="field"><label for="{phone}">Phone</label><input type="tel" id="{phone}" name="phone" placeholder="+1 (555) 555-5555"></div>
<div class="field"><label for="{linkedin}">LinkedIn Profile</label><input type="url" id="{linkedin}" name="urls[LinkedIn]"></div>
<div class="field"><label for="{github}">GitHub URL</label><input type="url" id="{github}" name="urls[Github ]"></div>
<div class="field"><label for="{address}">Street address</label><input type="text" id="{address}" name="job_application[address]"></div>
<div class="field"><label for="{city}">City</label><input type="text" id="{city}" name="job_application[city]"></div>
<div class="field"><label for="{zip}">Zip code</label><input type="text" id="{zip}" name="job_application[zip]"></div>
<div class="field"><label for="{auth}">Will you require visa sponsorship?</label><select id="{auth}" name="job_application[visa_sponsorship]"><option value="">--</option><option>Yes</option><option>No</option></select></div>
<div class="field"><label for="{why}">Why are you interested in this role?</label><textarea id="{why}" name="job_application[answers_attributes][0][text_value]" rows="5"></textarea></div>
<div class="field"><label for="{salary}">Expected annual salary (USD)</label><input type="number" id="{salary}" name="job_application[answers_attributes][1][text_value]"></div>
</form>"#,
        first = id("first"),
        last = id("last"),
        email = id("email"),
        phone = id("phone"),
        linkedin = id("linkedin"),
        github = id("github"),
        address = id("address"),
        city = id("city"),
        zip = id("zip"),
        auth = id("auth"),
        why = id("why"),
        salary = id("salary"),
    );
    page_shell(AtsVendor::Greenhouse, "Senior Software Engineer", opts, &body)
}

fn render_lever(opts: &FixtureOpts) -> String {
    // Production selectors observed in jeffistyping/workpls (Jeffrey Hui).
    // Lever uses a SINGLE `name` field combining first+last, not two
    // separate fields. `urls[LinkedIn]` / `urls[Github]` (capital G)
    // are the canonical names for the optional URL block — companies
    // sometimes type the trailing space `urls[Github ]` when adding
    // the field through Lever's admin, so we mirror that variant on
    // the github slot.
    let id = |s: &str| field_id(s, opts);
    let body = format!(
        r#"<form>
<div class="application-question"><label class="application-label" for="{full}">Full name</label><input type="text" id="{full}" name="name" required></div>
<div class="application-question"><input type="email" id="{email}" name="email" aria-label="Email" placeholder="Email"></div>
<div class="application-question"><input type="tel" id="{phone}" name="phone" aria-label="Phone" placeholder="Phone"></div>
<div class="application-question"><input type="url" id="{linkedin}" name="urls[LinkedIn]" aria-label="LinkedIn URL" placeholder="LinkedIn URL"></div>
<div class="application-question"><input type="url" id="{github}" name="urls[Github ]" aria-label="GitHub URL" placeholder="GitHub URL"></div>
<div class="application-question"><label class="application-label" for="{auth}">Are you authorized to work in the United States?</label><select id="{auth}" name="cards[a1234][field0]"><option value="">Choose</option><option>Yes</option><option>No</option></select></div>
<div class="application-question"><label class="application-label" for="{why}">What excites you about this role?</label><textarea id="{why}" name="comments" rows="4"></textarea></div>
<div class="application-question"><label class="application-label" for="{salary}">Salary expectation</label><input type="number" id="{salary}" name="cards[a1234][field1]"></div>
</form>"#,
        // Lever has one combined name field — we expose it under the
        // "first" slot for ID consistency; "last" is omitted from
        // expected_keys for Lever.
        full = id("first"),
        email = id("email"),
        phone = id("phone"),
        linkedin = id("linkedin"),
        github = id("github"),
        auth = id("auth"),
        why = id("why"),
        salary = id("salary"),
    );
    page_shell(AtsVendor::Lever, "Backend Engineer", opts, &body)
}

fn render_workday(opts: &FixtureOpts) -> String {
    // Production selectors observed in ubangura/Workday-Application-Automator
    // (Nathaniel Ubangura). Workday's canonical attribute is
    // `data-automation-id`, used everywhere — fields, page wrappers,
    // navigation buttons. Selects are `<button>` elements that open
    // dropdowns, NOT real `<select>` elements. The form is a multi-page
    // wizard wrapped in `div[data-automation-id="contactInformationPage"]`.
    let id = |s: &str| field_id(s, opts);
    let body = format!(
        r#"<div class="wd-container"><h2>My Information</h2>
<div data-automation-id="contactInformationPage"><form>
<div class="wd-form-row"><span class="wd-label">Legal Name (First)</span><input type="text" name="legalNameFirst" id="{first}" aria-label="First Name" data-automation-id="legalNameSection_firstName"></div>
<div class="wd-form-row"><span class="wd-label">Legal Name (Last)</span><input type="text" name="legalNameLast" id="{last}" aria-label="Last Name" data-automation-id="legalNameSection_lastName"></div>
<div class="wd-form-row"><span class="wd-label">Email Address</span><input type="email" name="primaryEmail" id="{email}" aria-label="Email" data-automation-id="email"></div>
<div class="wd-form-row"><span class="wd-label">Phone Device Type</span><button type="button" data-automation-id="phone-device-type" aria-label="Phone Device Type">Mobile ▾</button></div>
<div class="wd-form-row"><span class="wd-label">Phone Number</span><input type="tel" name="phoneNumber" id="{phone}" aria-label="Phone Number" data-automation-id="phone-number"></div>
<div class="wd-form-row"><span class="wd-label">Address Line 1</span><input type="text" name="addressLine1" id="{address}" aria-label="Street Address" data-automation-id="addressSection_addressLine1"></div>
<div class="wd-form-row"><span class="wd-label">City</span><input type="text" name="city" id="{city}" aria-label="City" data-automation-id="addressSection_city"></div>
<div class="wd-form-row"><span class="wd-label">Postal Code</span><input type="text" name="postalCode" id="{zip}" aria-label="Zip Code" data-automation-id="addressSection_postalCode"></div>
<div class="wd-form-row"><span class="wd-label">Country/Region</span><button type="button" data-automation-id="addressSection_countryRegion" aria-label="Country">United States ▾</button></div>
<div class="wd-form-row"><span class="wd-label">LinkedIn (Optional)</span><input type="url" name="websiteLinkedIn" id="{linkedin}" aria-label="LinkedIn URL"></div>
<div class="wd-form-row"><span class="wd-label">GitHub (Optional)</span><input type="url" name="websiteGitHub" id="{github}" aria-label="GitHub URL"></div>
<div class="wd-form-row"><span class="wd-label">Are you legally authorized to work in this country?</span><select name="workAuth" id="{auth}" aria-label="work authorization"><option>Yes</option><option>No</option></select></div>
<div class="wd-form-row"><span class="wd-label">Tell us about a recent project</span><textarea name="recentProject" id="{why}" rows="4"></textarea></div>
<div class="wd-form-row"><span class="wd-label">Expected compensation (USD)</span><input type="number" name="expectedCompensation" id="{salary}"></div>
<div class="wd-nav"><button type="button" data-automation-id="bottom-navigation-next-button">Save and Continue ›</button></div>
</form></div></div>"#,
        first = id("first"),
        last = id("last"),
        email = id("email"),
        phone = id("phone"),
        linkedin = id("linkedin"),
        github = id("github"),
        address = id("address"),
        city = id("city"),
        zip = id("zip"),
        auth = id("auth"),
        why = id("why"),
        salary = id("salary"),
    );
    page_shell(AtsVendor::Workday, "Workday — My Information", opts, &body)
}

fn render_icims(opts: &FixtureOpts) -> String {
    // iCIMS pattern: <fieldset> + <legend> for grouping; numeric IDs
    // typical (iCIMSField_1234); plain HTML form layout.
    let id = |s: &str| field_id(s, opts);
    let body = format!(
        r#"<form id="iCIMS_apply">
<fieldset><legend>Personal Information</legend>
<div class="iCIMSField"><label for="{first}">First Name</label><input type="text" id="{first}" name="iCIMSField_first"></div>
<div class="iCIMSField"><label for="{last}">Last Name</label><input type="text" id="{last}" name="iCIMSField_last"></div>
<div class="iCIMSField"><label for="{email}">Email</label><input type="email" id="{email}" name="iCIMSField_email"></div>
<div class="iCIMSField"><label for="{phone}">Phone</label><input type="tel" id="{phone}" name="iCIMSField_phone"></div>
</fieldset>
<fieldset><legend>Address</legend>
<div class="iCIMSField"><label for="{address}">Street</label><input type="text" id="{address}" name="iCIMSField_addr"></div>
<div class="iCIMSField"><label for="{city}">City</label><input type="text" id="{city}" name="iCIMSField_city"></div>
<div class="iCIMSField"><label for="{zip}">Postal Code</label><input type="text" id="{zip}" name="iCIMSField_zip"></div>
</fieldset>
<fieldset><legend>Online Presence</legend>
<div class="iCIMSField"><label for="{linkedin}">LinkedIn</label><input type="url" id="{linkedin}" name="iCIMSField_linkedin"></div>
<div class="iCIMSField"><label for="{github}">GitHub</label><input type="url" id="{github}" name="iCIMSField_github"></div>
</fieldset>
<fieldset><legend>Eligibility</legend>
<div class="iCIMSField"><label for="{auth}">Are you authorized to work in the US?</label><select id="{auth}" name="iCIMSField_workAuth"><option>Yes</option><option>No</option></select></div>
</fieldset>
<fieldset><legend>Additional</legend>
<div class="iCIMSField"><label for="{why}">Briefly describe why you're a fit</label><textarea id="{why}" name="iCIMSField_why"></textarea></div>
<div class="iCIMSField"><label for="{salary}">Salary expectation</label><input type="number" id="{salary}" name="iCIMSField_salary"></div>
</fieldset>
</form>"#,
        first = id("first"),
        last = id("last"),
        email = id("email"),
        phone = id("phone"),
        linkedin = id("linkedin"),
        github = id("github"),
        address = id("address"),
        city = id("city"),
        zip = id("zip"),
        auth = id("auth"),
        why = id("why"),
        salary = id("salary"),
    );
    page_shell(AtsVendor::Icims, "iCIMS — Apply", opts, &body)
}

fn render_ashby(opts: &FixtureOpts) -> String {
    // Ashby pattern: modern React-shaped, role="combobox" instead of
    // <select> for some categorical fields. We use real <select> for
    // consumer-test simplicity but mirror Ashby's visible structure.
    let id = |s: &str| field_id(s, opts);
    let body = format!(
        r#"<form class="ashby-application-form">
<div class="_field"><label for="{first}">First Name</label><input type="text" id="{first}" name="firstName"></div>
<div class="_field"><label for="{last}">Last Name</label><input type="text" id="{last}" name="lastName"></div>
<div class="_field"><label for="{email}">Email</label><input type="email" id="{email}" name="email"></div>
<div class="_field"><label for="{phone}">Phone</label><input type="tel" id="{phone}" name="phoneNumber"></div>
<div class="_field"><label for="{linkedin}">LinkedIn URL</label><input type="url" id="{linkedin}" name="linkedinUrl"></div>
<div class="_field"><label for="{github}">GitHub URL</label><input type="url" id="{github}" name="githubUrl"></div>
<div class="_field"><label for="{address}">Address</label><input type="text" id="{address}" name="address"></div>
<div class="_field"><label for="{city}">City</label><input type="text" id="{city}" name="city"></div>
<div class="_field"><label for="{zip}">Zip</label><input type="text" id="{zip}" name="postalCode"></div>
<div class="_field"><label for="{auth}">Work Authorization</label><select id="{auth}" name="workAuthorization" role="combobox"><option>I am authorized</option><option>I require sponsorship</option></select></div>
<div class="_field"><label for="{why}">Why this role?</label><textarea id="{why}" name="answer_why_role"></textarea></div>
<div class="_field"><label for="{salary}">Compensation expectations</label><input type="number" id="{salary}" name="compensationExpectation"></div>
</form>"#,
        first = id("first"),
        last = id("last"),
        email = id("email"),
        phone = id("phone"),
        linkedin = id("linkedin"),
        github = id("github"),
        address = id("address"),
        city = id("city"),
        zip = id("zip"),
        auth = id("auth"),
        why = id("why"),
        salary = id("salary"),
    );
    page_shell(AtsVendor::Ashby, "Ashby — Apply", opts, &body)
}

fn page_shell(vendor: AtsVendor, title: &str, opts: &FixtureOpts, body: &str) -> String {
    let css = vendor_css(vendor);
    let wrapped = hydration_wrapper(opts, body);
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><title>{title}</title><style>{css}</style></head><body>{wrapped}</body></html>"#
    )
}

fn vendor_css(vendor: AtsVendor) -> &'static str {
    match vendor {
        AtsVendor::Greenhouse => {
            "body{font:14px/1.5 system-ui;max-width:680px;margin:40px auto;padding:0 16px}\
             .field{margin:18px 0}label{display:block;font-weight:600;margin-bottom:6px;font-size:13px}\
             input,textarea,select{width:100%;padding:8px;border:1px solid #d0d7de;border-radius:6px;font:inherit;box-sizing:border-box}\
             .row{display:flex;gap:12px}.row>div{flex:1}"
        }
        AtsVendor::Lever => {
            "body{font:14px/1.4 'Inter',system-ui;max-width:600px;margin:30px auto;padding:0 20px}\
             .application-question{margin:16px 0}\
             .application-label{font-size:13px;font-weight:500;margin-bottom:6px;display:block}\
             input,textarea,select{width:100%;border:1px solid #ddd;padding:10px;border-radius:3px;box-sizing:border-box;font:inherit}"
        }
        AtsVendor::Workday => {
            "body{font:13px/1.5 'Helvetica Neue',sans-serif;background:#f1f1f1;margin:0}\
             .wd-container{background:#fff;max-width:720px;margin:30px auto;padding:32px;border:1px solid #e0e0e0}\
             h2{margin-top:0;font-size:18px;color:#0070d2}\
             .wd-form-row{margin:14px 0;display:flex;flex-direction:column}\
             .wd-form-row span.wd-label{font-size:12px;font-weight:600;margin-bottom:4px}\
             input,textarea,select{width:100%;border:1px solid #b8b8b8;padding:6px 8px;font-size:13px;box-sizing:border-box}"
        }
        AtsVendor::Icims => {
            "body{font:13px/1.4 Verdana,sans-serif;max-width:740px;margin:20px auto;padding:0 16px}\
             fieldset{border:1px solid #999;margin:14px 0;padding:12px 16px}\
             legend{font-weight:bold;padding:0 6px}\
             .iCIMSField{margin:10px 0}label{display:block;margin-bottom:4px}\
             input,textarea,select{width:100%;padding:6px;font:inherit;box-sizing:border-box}"
        }
        AtsVendor::Ashby => {
            "body{font:14px/1.5 'Inter','Segoe UI',sans-serif;max-width:660px;margin:30px auto;padding:0 16px;color:#0a0a0a}\
             ._field{margin:14px 0}label{display:block;font-size:13px;font-weight:500;margin-bottom:6px}\
             input,textarea,select{width:100%;padding:8px;border:1px solid #e5e5e5;border-radius:8px;font:inherit;box-sizing:border-box}"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_vendor_renders_nonempty_html() {
        for v in [
            AtsVendor::Greenhouse,
            AtsVendor::Lever,
            AtsVendor::Workday,
            AtsVendor::Icims,
            AtsVendor::Ashby,
        ] {
            let html = render(v, &FixtureOpts::default());
            assert!(html.starts_with("<!doctype html>"), "{v:?}: bad doctype");
            assert!(html.contains("</html>"), "{v:?}: missing </html>");
        }
    }

    #[test]
    fn vendor_label_unique_per_variant() {
        // Drift-guard: if a variant is added later, .label() must
        // return a unique non-empty string for it.
        let labels: Vec<&str> = [
            AtsVendor::Greenhouse,
            AtsVendor::Lever,
            AtsVendor::Workday,
            AtsVendor::Icims,
            AtsVendor::Ashby,
        ]
        .iter()
        .map(|v| v.label())
        .collect();
        let mut sorted = labels.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), labels.len(), "duplicate vendor labels");
        for l in &labels {
            assert!(!l.is_empty());
        }
    }

    #[test]
    fn dynamic_ids_change_field_id_format() {
        let stable = field_id("email", &FixtureOpts::default());
        let dynamic = field_id(
            "email",
            &FixtureOpts {
                dynamic_ids: true,
                ..Default::default()
            },
        );
        assert_eq!(stable, "email");
        assert!(dynamic.starts_with("input-"), "got {dynamic}");
        assert!(dynamic.ends_with("-email"));
    }

    #[test]
    fn dynamic_ids_are_deterministic_across_calls() {
        // Test snapshots must reproduce. djb2 hash of the slot name
        // is stable; UUIDs would not be.
        let a = field_id("phone", &FixtureOpts {
            dynamic_ids: true, ..Default::default()
        });
        let b = field_id("phone", &FixtureOpts {
            dynamic_ids: true, ..Default::default()
        });
        assert_eq!(a, b);
    }

    #[test]
    fn late_hydration_emits_settimeout() {
        let html = render(
            AtsVendor::Workday,
            &FixtureOpts {
                late_hydration_ms: Some(500),
                ..Default::default()
            },
        );
        assert!(html.contains("setTimeout"));
        assert!(html.contains("500"));
        // Body should NOT be inline — it should be the JSON string
        // assigned later. Quick check: form tag should NOT appear in
        // the raw HTML before hydration.
        let pre_script = html.split("<script>").next().unwrap();
        assert!(
            !pre_script.contains("<form"),
            "form rendered eagerly when late_hydration was set"
        );
    }

    #[test]
    fn rebuild_on_focus_emits_focusin_listener() {
        let html = render(
            AtsVendor::Workday,
            &FixtureOpts {
                rebuild_on_focus: true,
                ..Default::default()
            },
        );
        assert!(html.contains("focusin"));
    }

    #[test]
    fn expected_keys_cover_full_vocab_for_workday() {
        let pairs = expected_keys(AtsVendor::Workday, &FixtureOpts::default());
        // Workday should include the address sub-fields.
        let keys: Vec<&str> = pairs.iter().map(|(_, k)| *k).collect();
        for must_have in [
            "email",
            "phone",
            "full_name",
            "linkedin",
            "github",
            "address",
            "work_authorization",
            "freetext",
        ] {
            assert!(
                keys.contains(&must_have),
                "Workday expected_keys missing {must_have}"
            );
        }
    }

    #[test]
    fn lever_omits_address_subfields() {
        // Lever postings typically don't ask for address inline.
        let pairs = expected_keys(AtsVendor::Lever, &FixtureOpts::default());
        let ids: Vec<String> = pairs.iter().map(|(i, _)| i.clone()).collect();
        assert!(!ids.contains(&"address".to_string()));
        assert!(!ids.contains(&"city".to_string()));
        assert!(!ids.contains(&"zip".to_string()));
    }

    #[test]
    fn workday_uses_aria_label_carrying_semantic_meaning() {
        // The renderer must emit aria-label for Workday — that's the
        // signal the classifier relies on, since the visible <span>
        // can be ambiguous ("LinkedIn (Optional)" vs aria "LinkedIn URL").
        let html = render(AtsVendor::Workday, &FixtureOpts::default());
        assert!(html.contains(r#"aria-label="LinkedIn URL""#));
        assert!(html.contains(r#"aria-label="Email""#));
        assert!(html.contains(r#"aria-label="Phone Number""#));
    }

    #[test]
    fn html_contains_no_external_resources() {
        // Self-contained property — fixtures must not load network
        // resources, which would defeat the deterministic-test claim.
        for v in [
            AtsVendor::Greenhouse,
            AtsVendor::Lever,
            AtsVendor::Workday,
            AtsVendor::Icims,
            AtsVendor::Ashby,
        ] {
            let html = render(v, &FixtureOpts::default());
            assert!(!html.contains("<link"), "{v:?}: external <link>");
            assert!(!html.contains("<img"), "{v:?}: <img> with src");
            assert!(!html.contains(r#"src="http"#), "{v:?}: external script");
        }
    }

    #[test]
    fn json_escape_handles_quotes_and_newlines() {
        let escaped = json_escape_string("a\"b\nc</script>");
        assert!(escaped.starts_with('"') && escaped.ends_with('"'));
        assert!(escaped.contains("\\\""));
        assert!(escaped.contains("\\n"));
        // </script> is the XSS-relevant case: we escape '/' so an
        // injected </script> can't break out of the wrapping <script>.
        assert!(!escaped.contains("</"));
    }
}
