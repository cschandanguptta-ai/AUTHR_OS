// AUTHR_OS — Craft Catalog (contract v1.3): the static, deterministic,
// fully-offline knowledge base that turns the runtime into a one-person
// authoring + publishing house.
//
//   FORMATS   — 25 project formats (novel → picture book → screenplay → memoir)
//   PERSONAS  — 37 bestseller-archetype style personas (craft DNA, not
//               impersonation: POV, tense, rhythm, pacing, an LLM directive)
//   ARCS      — 18 plot structures with real beat sheets (Save the Cat,
//               Hero's Journey, Kishōtenketsu, Story Circle, nonfiction…)
//   PRICING   — Amazon KDP royalty constants (planning approximation)
//
// Everything here is data, not a mock: lookups are pure functions over
// frozen tables and every id is unit-tested for uniqueness (mod tests).

use serde_json::{json, Value};

// ─────────────────────────────────────────────────────────────────────────────
// Project formats — what shape of book is being written. Drives default word
// counts / chapter targets (craft_apply) and KDP word-band advice (kdp_check).
// ─────────────────────────────────────────────────────────────────────────────

pub fn formats() -> Vec<Value> {
    vec![
        // ── Fiction ──────────────────────────────────────────────────────────
        json!({"id":"NOVEL","label":"Novel","category":"FICTION","default_word_count":80000,"default_chapters":25,"unit":"chapters","kdp_note":"Fiction sweet spot 50k-100k words; 70% royalty tier applies to $2.99-$9.99."}),
        json!({"id":"NOVELLA","label":"Novella","category":"FICTION","default_word_count":25000,"default_chapters":10,"unit":"chapters","kdp_note":"20k-50k words; strong Kindle Unlimited performer."}),
        json!({"id":"NOVELETTE","label":"Novelette","category":"FICTION","default_word_count":12000,"default_chapters":5,"unit":"chapters","kdp_note":"7.5k-20k words; often bundled into collections."}),
        json!({"id":"SHORT_STORY","label":"Short Story","category":"FICTION","default_word_count":6000,"default_chapters":1,"unit":"chapters","kdp_note":"Under 7.5k words; publish via collections or platforms, not a standalone KDP paperback."}),
        json!({"id":"SHORT_STORY_COLLECTION","label":"Short Story Collection","category":"FICTION","default_word_count":60000,"default_chapters":15,"unit":"chapters","kdp_note":"15+ stories; front matter carries a table of contents."}),
        json!({"id":"ANTHOLOGY","label":"Multi-Author Anthology","category":"FICTION","default_word_count":80000,"default_chapters":16,"unit":"chapters","kdp_note":"Per-author bylines need contributor pages in front matter."}),
        json!({"id":"SERIALIZED_WEBNOVEL","label":"Serialized Web Novel","category":"FICTION","default_word_count":200000,"default_chapters":60,"unit":"episodes","kdp_note":"Serialized on Wattpad/Royal Road then boxed into KDP volumes."}),
        json!({"id":"FLASH_COLLECTION","label":"Flash Fiction Collection","category":"FICTION","default_word_count":30000,"default_chapters":30,"unit":"chapters","kdp_note":"Each chapter is one flash piece (under 1k words)."}),
        json!({"id":"YOUNG_ADULT_NOVEL","label":"Young Adult (YA) Novel","category":"FICTION","default_word_count":65000,"default_chapters":25,"unit":"chapters","kdp_note":"YA fantasy/thriller tolerates 70k-90k; contemporary 55k-70k."}),
        // ── Kids ─────────────────────────────────────────────────────────────
        json!({"id":"MIDDLE_GRADE_NOVEL","label":"Middle-Grade Novel","category":"KIDS","default_word_count":35000,"default_chapters":20,"unit":"chapters","kdp_note":"30k-50k words; ages 8-12; chapter-level cliffhangers matter."}),
        json!({"id":"CHAPTER_BOOK","label":"Early Chapter Book","category":"KIDS","default_word_count":9000,"default_chapters":8,"unit":"chapters","kdp_note":"Ages 6-9; short chapters, high illustration ratio."}),
        json!({"id":"PICTURE_BOOK","label":"Picture Book","category":"KIDS","default_word_count":600,"default_chapters":1,"unit":"spreads","kdp_note":"500-800 words; KDP paperback needs full bleed 8.5x8.5 setup."}),
        // ── Nonfiction ───────────────────────────────────────────────────────
        json!({"id":"NF_TRADE_BOOK","label":"Narrative Non-Fiction","category":"NONFICTION","default_word_count":55000,"default_chapters":12,"unit":"chapters","kdp_note":"Scene-driven true stories; back matter carries sources."}),
        json!({"id":"MEMOIR","label":"Memoir","category":"NONFICTION","default_word_count":85000,"default_chapters":20,"unit":"chapters","kdp_note":"70k-100k words; an arc-shaped slice of a life, not autobiography."}),
        json!({"id":"SELF_HELP","label":"Self-Help / Personal Development","category":"NONFICTION","default_word_count":50000,"default_chapters":12,"unit":"chapters","kdp_note":"One transformation per chapter; workbook pages boost value."}),
        json!({"id":"BUSINESS_BOOK","label":"Business & Leadership","category":"NONFICTION","default_word_count":45000,"default_chapters":12,"unit":"chapters","kdp_note":"Case study + playbook chapters; 40k-55k words."}),
        json!({"id":"BIOGRAPHY","label":"Biography","category":"NONFICTION","default_word_count":90000,"default_chapters":20,"unit":"chapters","kdp_note":"Chronological with set-piece scenes; permissions for quotes."}),
        json!({"id":"NARRATIVE_HISTORY","label":"Narrative History","category":"NONFICTION","default_word_count":95000,"default_chapters":20,"unit":"chapters","kdp_note":"Immersive history; maps and notes sections expected."}),
        json!({"id":"HOW_TO_GUIDE","label":"How-To Guide","category":"NONFICTION","default_word_count":40000,"default_chapters":10,"unit":"chapters","kdp_note":"Step-by-step chapters with end-of-chapter checklists."}),
        json!({"id":"ESSAY_COLLECTION","label":"Essay Collection","category":"NONFICTION","default_word_count":55000,"default_chapters":15,"unit":"chapters","kdp_note":"Standalone essays under one through-line."}),
        json!({"id":"ACADEMIC_MONOGRAPH","label":"Academic Monograph","category":"NONFICTION","default_word_count":80000,"default_chapters":10,"unit":"chapters","kdp_note":"Peer-review register; DOI/ISBN and citation style required."}),
        // ── Poetry ───────────────────────────────────────────────────────────
        json!({"id":"POETRY_COLLECTION","label":"Poetry Collection","category":"POETRY","default_word_count":12000,"default_chapters":32,"unit":"poems","kdp_note":"One poem per chapter keeps EPUB overlays and TOC clean."}),
        // ── Scripts ──────────────────────────────────────────────────────────
        json!({"id":"SCREENPLAY","label":"Feature Screenplay","category":"SCRIPT","default_word_count":20000,"default_chapters":40,"unit":"scenes","kdp_note":"~1 page per minute; sell via contests/cover pages, not KDP ebook."}),
        json!({"id":"STAGE_PLAY","label":"Stage Play","category":"SCRIPT","default_word_count":14000,"default_chapters":12,"unit":"scenes","kdp_note":"Act/scene structure; licensing via theatrical publishers."}),
        json!({"id":"GRAPHIC_NOVEL_SCRIPT","label":"Graphic Novel Script","category":"SCRIPT","default_word_count":18000,"default_chapters":20,"unit":"scenes","kdp_note":"Page-by-page panel descriptions for the art team."}),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Style personas — bestseller-archetype craft DNA. These are authoring
// archetypes ("in the tradition of" a bestseller family), described by
// observable craft attributes plus a directive that `local_llm_generate`
// prepends to every generation prompt when the persona is active.
// ─────────────────────────────────────────────────────────────────────────────

pub fn personas() -> Vec<Value> {
    vec![
        // ── Thriller / Suspense / Crime ──────────────────────────────────────
        json!({"id":"EDGE_THRILLER","label":"Edge-of-the-Seat Thriller","family":"Thriller","pov":"close third","tense":"past","rhythm":"short, staccato; chapters under ~1.5k words","diction":"concrete verbs, zero padding","pacing":"relentless","dialogue_share":0.35,"signature":"every chapter ends on a hook; ticking clocks; POV switches at cliff edges","directive":"Write high-velocity commercial suspense: short punchy sentences, hard chapter-end hooks, present physical stakes in every scene, minimal philosophizing.","best_for":["NOVEL","SERIALIZED_WEBNOVEL","NOVELLA"]}),
        json!({"id":"TECHNO_THRILLER","label":"Techno-Thriller","family":"Thriller","pov":"third multiple","tense":"past","rhythm":"procedural setups accelerating into action","diction":"jargon-credible, hardware-real","pacing":"accelerating","dialogue_share":0.35,"signature":"ops-room scenes, technical detail that sells the threat","directive":"Write a techno-thriller: credible technical procedure, institutional detail, threat escalation validated by evidence on the page.","best_for":["NOVEL"]}),
        json!({"id":"HARDBOILED_NOIR","label":"Hardboiled Noir","family":"Crime","pov":"first","tense":"past","rhythm":"terse, rhythmic, simile-laced","diction":"slang, hard cuts","pacing":"bruising","dialogue_share":0.40,"signature":"weathered PI narration, moral fog, city-as-character","directive":"Write hardboiled noir: first-person terse narration, sharp similes, corruption on every corner, violence with consequences.","best_for":["NOVEL","NOVELLA"]}),
        json!({"id":"CRIME_PROCEDURAL","label":"Police Procedural","family":"Crime","pov":"third","tense":"past","rhythm":"methodical","diction":"forensic, technical","pacing":"steady","dialogue_share":0.40,"signature":"interviews, evidence chains, jurisdiction friction","directive":"Write a police procedural: accurate investigative sequence, interview-driven scenes, chain-of-evidence logic the reader can audit.","best_for":["NOVEL","SERIALIZED_WEBNOVEL"]}),
        json!({"id":"CLASSIC_WHODUNIT","label":"Fair-Play Whodunit","family":"Mystery","pov":"third","tense":"past","rhythm":"measured","diction":"precise, period-flexible","pacing":"steady","dialogue_share":0.45,"signature":"clues planted in plain sight, closed circle of suspects, fair solution","directive":"Write a fair-play whodunit: every clue shown to the reader, red herrings honest, the detective reasons on-page before the reveal.","best_for":["NOVEL","NOVELLA"]}),
        json!({"id":"COZY_MYSTERY","label":"Cozy Mystery","family":"Mystery","pov":"first or close third","tense":"past","rhythm":"light, warm","diction":"conversational","pacing":"gentle","dialogue_share":0.50,"signature":"amateur sleuth, small-town texture, low on-page gore","directive":"Write a cozy mystery: charming setting, amateur sleuth with a real day job, community cast, violence off-page, puzzle front and center.","best_for":["NOVEL","SERIALIZED_WEBNOVEL"]}),
        json!({"id":"SPY_COLD_WAR","label":"Mole-Hunt Espionage","family":"Spy","pov":"third","tense":"past","rhythm":"quiet rooms, long shadows","diction":"bureaucratic irony","pacing":"slow-burn","dialogue_share":0.40,"signature":"loyalty tests, tradecraft detail, ambiguity of motive","directive":"Write espionage in the mole-hunt tradition: institutional suspicion, tradecraft realism, betrayal that recontextualizes earlier scenes.","best_for":["NOVEL"]}),
        // ── Horror ───────────────────────────────────────────────────────────
        json!({"id":"SLOW_BURN_HORROR","label":"Slow-Burn Horror","family":"Horror","pov":"close third","tense":"past","rhythm":"long build, sudden short","diction":"sensory, concrete","pacing":"creeping","dialogue_share":0.25,"signature":"normality curdling; dread carried by mundane detail","directive":"Write slow-burn horror: domesticate the mundane, escalate sensory wrongness, withhold the threat's shape as long as possible.","best_for":["NOVEL","NOVELLA"]}),
        // ── Fantasy ──────────────────────────────────────────────────────────
        json!({"id":"EPIC_FANTASY","label":"Epic Fantasy Tapestry","family":"Fantasy","pov":"multi-POV third","tense":"past","rhythm":"expansive","diction":"elevated, invented lexicon","pacing":"sweeping","dialogue_share":0.35,"signature":"interwoven storylines, cultures with internal logic","directive":"Write epic fantasy: multiple POV threads that collide, deep world logic (costs, politics, magic), historical weight in every place name.","best_for":["NOVEL","SERIALIZED_WEBNOVEL"]}),
        json!({"id":"PROGRESSION_FANTASY","label":"Progression Fantasy","family":"Fantasy","pov":"close third","tense":"past","rhythm":"goal-training-payoff loops","diction":"clear, kinetic","pacing":"rising","dialogue_share":0.35,"signature":"legible power systems, earned escalations, quantified growth","directive":"Write progression fantasy: clear magic/power systems with costs, visible character advancement, each arc paying off trained skills.","best_for":["NOVEL","SERIALIZED_WEBNOVEL"]}),
        json!({"id":"SWORD_SORCERY","label":"Sword & Sorcery","family":"Fantasy","pov":"third","tense":"past","rhythm":"pulpy","diction":"archaic-flash","pacing":"rapid","dialogue_share":0.40,"signature":"mercenary heroes, loot and ruin, doom-taglines","directive":"Write sword & sorcery: muscular prose, opportunistic heroes, ancient evils, momentum over exposition.","best_for":["NOVELETTE","SHORT_STORY_COLLECTION"]}),
        json!({"id":"COZY_FANTASY","label":"Cozy Slice-of-Life Fantasy","family":"Fantasy","pov":"close third","tense":"past","rhythm":"warm, episodic","diction":"gentle humor","pacing":"low-heat","dialogue_share":0.45,"signature":"tea-shop stakes, found family, comfort with tiny frissons","directive":"Write cozy fantasy: low-stakes warmth, food and craft detail, community problem-solving, one gentle mystery per episode.","best_for":["NOVEL","SERIALIZED_WEBNOVEL"]}),
        json!({"id":"FAIRYTALE_RETOLDING","label":"Fairytale Retelling","family":"Fantasy","pov":"third","tense":"past","rhythm":"storybook cadence; threes and oaths","diction":"lyrical-simple","pacing":"lilted","dialogue_share":0.40,"signature":"bargains, curses with grammar, transformed familiar motifs","directive":"Write a fairytale retelling: ritual cadence, rule-bound magic, the familiar tale bent until it surprises.","best_for":["NOVEL","NOVELETTE"]}),
        // ── Science fiction ──────────────────────────────────────────────────
        json!({"id":"SPACE_OPERA","label":"Space Opera","family":"Sci-Fi","pov":"third multiple","tense":"past","rhythm":"cinematic","diction":"bold names, galactic scale","pacing":"sweeping","dialogue_share":0.40,"signature":"empires, ships with souls, diplomacy at gunpoint","directive":"Write space opera: vast canvas, charismatic crews, political stakes, set-piece action that still honors character logic.","best_for":["NOVEL","SERIALIZED_WEBNOVEL"]}),
        json!({"id":"HARD_SF","label":"Hard Science Fiction","family":"Sci-Fi","pov":"third","tense":"past","rhythm":"exact","diction":"technical, explicated","pacing":"methodical","dialogue_share":0.30,"signature":"constraint-driven plot; the science is the antagonist","directive":"Write hard SF: established constraints, no unearned miracles, problem-solving on the page, numbers you can stand behind.","best_for":["NOVEL","NOVELLA"]}),
        json!({"id":"DYSTOPIAN_VOICE","label":"Dystopian Restraint","family":"Sci-Fi","pov":"third or first","tense":"past or present","rhythm":"clipped","diction":"clinical","pacing":"oppressive","dialogue_share":0.35,"signature":"systems that grind people; language control; small rebellions","directive":"Write dystopia: institutional language on the page, the cost of the system visible in small deprivations, hope rationed but present.","best_for":["NOVEL","YOUNG_ADULT_NOVEL"]}),
        json!({"id":"LITRPG_SYSTEM","label":"LitRPG System Voice","family":"LitRPG","pov":"first or third","tense":"past","rhythm":"scene interleaved with stat blocks","diction":"gamer-plain","pacing":"grinding","dialogue_share":0.45,"signature":"level-ups, skill windows, visible progression math","directive":"Write LitRPG: interleaved system windows, crisp combat choreography, progression that respects its own rules.","best_for":["SERIALIZED_WEBNOVEL","NOVEL"]}),
        // ── Romance ──────────────────────────────────────────────────────────
        json!({"id":"ROMANTIC_TENSION","label":"Dual-POV Romance","family":"Romance","pov":"alternating first","tense":"present (or past)","rhythm":"beat-timed; alternating POVs","diction":"contemporary, emotive","pacing":"rising","dialogue_share":0.55,"signature":"both sides of the attraction; trope-aware but sincere","directive":"Write dual-POV romance: alternating viewpoints with distinct voices, physical tension on the page, emotional beats that compound.","best_for":["NOVEL","NOVELLA"]}),
        json!({"id":"REGENCY_ROMANCE","label":"Regency Wit","family":"Historical Romance","pov":"third","tense":"past","rhythm":"irony-led, period-true","diction":"period diction","pacing":"measured","dialogue_share":0.50,"signature":"drawing-room repartee, propriety as pressure","directive":"Write Regency romance: sparkling period dialogue, social rules as antagonists, longing expressed through constraint.","best_for":["NOVEL","NOVELLA"]}),
        json!({"id":"PARANORMAL_ROMANCE","label":"Paranormal Romance","family":"Romance","pov":"first","tense":"present","rhythm":"fevered","diction":"lush","pacing":"high-heat","dialogue_share":0.55,"signature":"love across the veil; bond politics","directive":"Write paranormal romance: supernatural rules with costs, fated-but-fraught attraction, heat that serves the emotional arc.","best_for":["NOVEL","SERIALIZED_WEBNOVEL"]}),
        // ── Literary ─────────────────────────────────────────────────────────
        json!({"id":"LITERARY_INTERIOR","label":"Literary Interior","family":"Literary","pov":"third or first","tense":"past","rhythm":"long, recursive, image-anchored","diction":"layered","pacing":"patient","dialogue_share":0.25,"signature":"consciousness rendered; the sentence as instrument","directive":"Write literary fiction: interior consciousness, controlled metaphor, sentences that reward re-reading, meaning accumulating in images.","best_for":["NOVEL","NOVELLA","ESSAY_COLLECTION"]}),
        json!({"id":"PLAIN_MINIMALIST","label":"Iceberg Minimalist","family":"Literary","pov":"third","tense":"past","rhythm":"short declaratives","diction":"plain, denotative","pacing":"spare","dialogue_share":0.35,"signature":"subtext carries the scene; omission as technique","directive":"Write minimalist fiction: surface plainness, emotional payload under the waterline, never name the feeling the scene proves.","best_for":["NOVELETTE","SHORT_STORY_COLLECTION"]}),
        json!({"id":"MAGICAL_REALISM","label":"Magical Realism","family":"Literary","pov":"third","tense":"past","rhythm":"flowing","diction":"mythic everyday","pacing":"flowing","dialogue_share":0.30,"signature":"wonder delivered deadpan; family/time loops","directive":"Write magical realism: the impossible narrated as household fact, generational time, wonder without winking.","best_for":["NOVEL"]}),
        json!({"id":"SOUTHERN_GOTHIC","label":"Southern Gothic","family":"Literary","pov":"third","tense":"past","rhythm":"humid, circling","diction":"dialect-aware","pacing":"decaying","dialogue_share":0.35,"signature":"place as fate; rot beneath hospitality","directive":"Write Southern Gothic: setting as moral weather, family history as haunting, grotesques rendered with compassion.","best_for":["NOVEL"]}),
        json!({"id":"WESTERN_FRONTIER","label":"Frontier Western","family":"Western","pov":"third","tense":"past","rhythm":"laconic","diction":"spare, period","pacing":"deliberate","dialogue_share":0.40,"signature":"land as law; violence brief and costly","directive":"Write a western: laconic prose, landscape with agency, honor codes tested by hard choices.","best_for":["NOVEL","NOVELLA"]}),
        json!({"id":"SAGA_SWEEP","label":"Multigenerational Saga","family":"Family Saga","pov":"third multiple","tense":"past","rhythm":"generational set-pieces","diction":"rich","pacing":"measured","dialogue_share":0.40,"signature":"time skips, inherited consequences, dynasty logic","directive":"Write a family saga: generational structure, inherited secrets, era texture, consequences compounding across decades.","best_for":["NOVEL"]}),
        // ── Young audiences ──────────────────────────────────────────────────
        json!({"id":"YA_FIRST_PERSON","label":"Contemporary YA Voice","family":"YA","pov":"first","tense":"present","rhythm":"breathy, short","diction":"slang-aware, now","pacing":"propulsive","dialogue_share":0.55,"signature":"identity stakes; adults just off-center; first-love voltage","directive":"Write contemporary YA: immediate first-person voice, emotional stakes first, adults believable but not decisive, zero condescension.","best_for":["YOUNG_ADULT_NOVEL"]}),
        json!({"id":"MIDDLE_GRADE_ADV","label":"Middle-Grade Adventure","family":"Middle Grade","pov":"third or first","tense":"past","rhythm":"chapter-level cliffhangers","diction":"clear, vivid","pacing":"questing","dialogue_share":0.50,"signature":"kid competence, adult world at arm's length","directive":"Write middle-grade adventure: kid-protagonist agency, sensory fun, one big-hearted theme, chapter ends that demand one more.","best_for":["MIDDLE_GRADE_NOVEL","CHAPTER_BOOK"]}),
        json!({"id":"HUMOR_COMMERCIAL","label":"Comic Commercial Voice","family":"Humor","pov":"third or first","tense":"past","rhythm":"escalation with footnote gags","diction":"playful","pacing":"bouncy","dialogue_share":0.50,"signature":"runners, callbacks, absurdity with internal logic","directive":"Write commercial humor: escalating absurdity with strict internal logic, runners and callbacks, jokes that reveal character.","best_for":["NOVEL","SERIALIZED_WEBNOVEL"]}),
        // ── Nonfiction voices ────────────────────────────────────────────────
        json!({"id":"DIRECT_SELF_HELP","label":"Direct Self-Help Coach","family":"Self-Help","pov":"second","tense":"present","rhythm":"claim-story-framework-action","diction":"punchy plain","pacing":"brisk","dialogue_share":0.10,"signature":"named frameworks, end-of-chapter practices","directive":"Write self-help directly: speak to the reader as 'you', one transformation per chapter, story to framework to practice.","best_for":["SELF_HELP","HOW_TO_GUIDE"]}),
        json!({"id":"NARRATIVE_NONFICTION","label":"Immersive Narrative Non-Fiction","family":"Narrative NF","pov":"third","tense":"past","rhythm":"scene-driven reportage","diction":"precise, sourced","pacing":"alternating scene/summary","dialogue_share":0.20,"signature":"reconstructed scenes, sourcing that holds up","directive":"Write narrative non-fiction: reconstructed scenes with documented detail, characters over statistics, facts the reader can check.","best_for":["NF_TRADE_BOOK","BIOGRAPHY","NARRATIVE_HISTORY"]}),
        json!({"id":"MEMOIR_INTIMATE","label":"Intimate Memoir","family":"Memoir","pov":"first","tense":"past","rhythm":"reflective","diction":"textured","pacing":"meandering converging","dialogue_share":0.20,"signature":"the past self seen with mercy; scene + meditation","directive":"Write memoir: first-person intimacy, scenes that carry theme, the narrator's present understanding threading the past.","best_for":["MEMOIR","ESSAY_COLLECTION"]}),
        json!({"id":"THOUGHT_LEADER","label":"Big-Idea Essayist","family":"Ideas","pov":"first or third","tense":"present","rhythm":"anecdote-claim-evidence-twist","diction":"accessible-smart","pacing":"measured","dialogue_share":0.10,"signature":"counterintuitive claims earned by stories","directive":"Write big-idea non-fiction: a hook anecdote, a counterintuitive claim, layered evidence, honest counterarguments.","best_for":["BUSINESS_BOOK","NF_TRADE_BOOK"]}),
        json!({"id":"BUSINESS_PRACTICAL","label":"Playbook Business Voice","family":"Business","pov":"second or third","tense":"present","rhythm":"case-playbook-checklist","diction":"crisp","pacing":"structured","dialogue_share":0.10,"signature":"operators' specifics, metrics, repeatable steps","directive":"Write business non-fiction: one case per chapter, distill the playbook, end with a checklist the reader can run Monday.","best_for":["BUSINESS_BOOK","HOW_TO_GUIDE"]}),
        json!({"id":"ACADEMIC_RIGOROUS","label":"Academic Rigor","family":"Academic","pov":"third","tense":"present","rhythm":"hedged, formal","diction":"register-high","pacing":"dense","dialogue_share":0.05,"signature":"citations, taxonomy, limitations sections","directive":"Write academically: precise hedged claims, defined terms, citation-aware argumentation, explicit limitations.","best_for":["ACADEMIC_MONOGRAPH"]}),
        json!({"id":"JOURNALISTIC_CLEAR","label":"Explanatory Journalist","family":"Non-Fiction","pov":"third","tense":"present","rhythm":"inverted pyramid","diction":"neutral plain","pacing":"steady","dialogue_share":0.15,"signature":"quotes as evidence; structure over ornament","directive":"Write explanatory non-fiction: lead with the answer, evidence in layers, neutral tone, quotes that carry weight.","best_for":["NF_TRADE_BOOK","ESSAY_COLLECTION"]}),
        json!({"id":"SPIRITUAL_WARM","label":"Contemplative Guide","family":"Spirituality","pov":"second","tense":"present","rhythm":"breath-paced","diction":"gentle","pacing":"calm","dialogue_share":0.10,"signature":"benedictory cadence; practice anchored in the body","directive":"Write spiritual non-fiction: second-person warmth, breath-paced rhythm, practices the reader can do while reading.","best_for":["SELF_HELP","NF_TRADE_BOOK"]}),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Plot arcs — 18 structures with real beat sheets. `legacy_structure` maps the
// four arcs whose ids are frozen inside the books.narrative_structure CHECK
// constraint; the other 14 live only in craft_state (books keep a legal
// legacy value, the arc truth lives in the craft layer).
// ─────────────────────────────────────────────────────────────────────────────

pub fn arcs() -> Vec<Value> {
    vec![
        json!({"id":"SAVE_THE_CAT","label":"Save the Cat (15 Beats)","lineage":"Blake Snyder","legacy_structure":"SAVE_THE_CAT","best_for":["NOVEL","YOUNG_ADULT_NOVEL","SCREENPLAY"],"beats":[
            {"id":"opening_image","label":"Opening Image","guidance":"A snapshot of the flawed before-world and the hero inside it."},
            {"id":"theme_stated","label":"Theme Stated","guidance":"Someone states what the hero needs to learn; it lands later."},
            {"id":"setup","label":"Set-Up","guidance":"Life, world and every character that needs fixing — plant everything that will be repaired."},
            {"id":"catalyst","label":"Catalyst","guidance":"The disruptive event that shatters the status quo."},
            {"id":"debate","label":"Debate","guidance":"The hero argues with the change; stakes and reluctance on display."},
            {"id":"break_into_two","label":"Break into Two","guidance":"An active choice enters the upside-down world of Act Two."},
            {"id":"b_story","label":"B Story","guidance":"The love/mentor thread that carries the theme."},
            {"id":"fun_and_games","label":"Fun and Games","guidance":"The promise of the premise — why the reader came."},
            {"id":"midpoint","label":"Midpoint","guidance":"False victory or false defeat; stakes rise, hero commits."},
            {"id":"bad_guys_close_in","label":"Bad Guys Close In","guidance":"External pressure and internal cracks tighten together."},
            {"id":"all_is_lost","label":"All Is Lost","guidance":"The whiff of death; the bottom."},
            {"id":"dark_night","label":"Dark Night of the Soul","guidance":"Grief, reflection, the unworkable truth faced."},
            {"id":"break_into_three","label":"Break into Three","guidance":"A + B stories combine into the real solution."},
            {"id":"finale","label":"Finale","guidance":"Storm the castle in five steps; prove the change."},
            {"id":"final_image","label":"Final Image","guidance":"Mirror of the opening image — the after-world."}]}),
        json!({"id":"THREE_ACT","label":"Three-Act Structure","lineage":"Aristotle → Syd Field","legacy_structure":"THREE_ACT","best_for":["NOVEL","NOVELLA","MEMOIR"],"beats":[
            {"id":"inciting_incident","label":"Inciting Incident","guidance":"The event that starts the story clock."},
            {"id":"plot_point_1","label":"Plot Point One","guidance":"The door swings shut behind the hero."},
            {"id":"pinch_1","label":"First Pinch","guidance":"Antagonistic force demonstrates real cost."},
            {"id":"midpoint","label":"Midpoint","guidance":"Shift from reaction to action; truth or lie clarified."},
            {"id":"pinch_2","label":"Second Pinch","guidance":"Worst pressure yet; a mentor falls or a plan dies."},
            {"id":"plot_point_2","label":"Plot Point Two","guidance":"The final piece (idea, ally, resolve) enters."},
            {"id":"climax_buildup","label":"Climax Build-Up","guidance":"Forces converge; no retreat remains."},
            {"id":"climax","label":"Climax","guidance":"The decisive confrontation earns the theme."},
            {"id":"resolution","label":"Resolution","guidance":"The new equilibrium, shown not summarized."}]}),
        json!({"id":"HEROS_JOURNEY","label":"Hero's Journey (12 Stages)","lineage":"Campbell / Vogler","legacy_structure":"HEROS_JOURNEY","best_for":["NOVEL","MIDDLE_GRADE_NOVEL","GRAPHIC_NOVEL_SCRIPT"],"beats":[
            {"id":"ordinary_world","label":"Ordinary World","guidance":"The hero's normal, its lack visible."},
            {"id":"call_to_adventure","label":"Call to Adventure","guidance":"A problem or invitation disrupts the ordinary."},
            {"id":"refusal","label":"Refusal of the Call","guidance":"Fear or duty says no; the cost of yes is priced."},
            {"id":"mentor","label":"Meeting the Mentor","guidance":"Guidance, gift or training arrives."},
            {"id":"crossing_threshold","label":"Crossing the Threshold","guidance":"The special world is entered; rules change."},
            {"id":"tests_allies_enemies","label":"Tests, Allies, Enemies","guidance":"The new world is mapped through trial."},
            {"id":"inmost_cave","label":"Approach the Inmost Cave","guidance":"Preparation for the central ordeal."},
            {"id":"ordeal","label":"The Ordeal","guidance":"Death-and-rebirth at the story's center."},
            {"id":"reward","label":"Reward (Seizing the Sword)","guidance":"The prize taken — with strings attached."},
            {"id":"road_back","label":"The Road Back","guidance":"Chase or consequence drives the return."},
            {"id":"resurrection","label":"Resurrection","guidance":"The final test proves the transformation."},
            {"id":"return_elixir","label":"Return with the Elixir","guidance":"The boon that heals the ordinary world."}]}),
        json!({"id":"FICHTEAN_CURVE","label":"Fichtean Curve","lineage":"John Gardner's crystallization","legacy_structure":"FICHTEAN_CURVE","best_for":["NOVEL","SERIALIZED_WEBNOVEL"],"beats":[
            {"id":"crisis_one","label":"Crisis One","guidance":"Open in trouble; exposition woven into action."},
            {"id":"crisis_two","label":"Crisis Two","guidance":"Each crisis worse; backstory drip-fed."},
            {"id":"crisis_three","label":"Crisis Three","guidance":"The true shape of the conflict surfaces."},
            {"id":"crisis_four","label":"Crisis Four","guidance":"A door closes; options narrow."},
            {"id":"crisis_five","label":"Crisis Five","guidance":"The last crisis before the peak; stakes maximal."},
            {"id":"climax","label":"Climax","guidance":"The wave crests and breaks."},
            {"id":"resolution","label":"Falling Action","guidance":"Aftermath that certifies the change."}]}),
        json!({"id":"SEVEN_POINT","label":"Seven-Point Structure","lineage":"Dan Wells","legacy_structure":null,"best_for":["NOVEL","NOVELLA"],"beats":[
            {"id":"hook","label":"Hook","guidance":"The starting opposite-state of the hero."},
            {"id":"plot_turn_1","label":"Plot Turn 1","guidance":"The call; the world changes hands."},
            {"id":"pinch_1","label":"Pinch 1","guidance":"Pressure applied; the enemy is real."},
            {"id":"midpoint","label":"Midpoint","guidance":"Reaction becomes action; knowledge shifts."},
            {"id":"pinch_2","label":"Pinch 2","guidance":"The price is paid; all seems lost."},
            {"id":"plot_turn_2","label":"Plot Turn 2","guidance":"The final piece needed to win arrives."},
            {"id":"resolution","label":"Resolution","guidance":"The goal as redefined by the journey."}]}),
        json!({"id":"STORY_CIRCLE","label":"Harmon Story Circle","lineage":"Dan Harmon","legacy_structure":null,"best_for":["SERIALIZED_WEBNOVEL","NOVEL","SCREENPLAY"],"beats":[
            {"id":"you","label":"You","guidance":"A character in a zone of comfort."},
            {"id":"need","label":"Need","guidance":"They want something."},
            {"id":"go","label":"Go","guidance":"They enter an unfamiliar situation."},
            {"id":"search","label":"Search","guidance":"They adapt to it."},
            {"id":"find","label":"Find","guidance":"They get what they wanted."},
            {"id":"take","label":"Take","guidance":"They pay a heavy price for it."},
            {"id":"return","label":"Return","guidance":"They return to the familiar situation."},
            {"id":"change","label":"Change","guidance":"Having changed, they can begin again."}]}),
        json!({"id":"FREYTAG_PYRAMID","label":"Freytag's Pyramid","lineage":"Gustav Freytag","legacy_structure":null,"best_for":["NOVEL"],"beats":[
            {"id":"exposition","label":"Exposition","guidance":"World, people, tension introduced."},
            {"id":"inciting_incident","label":"Inciting Moment","guidance":"Exciting force disturbs the order."},
            {"id":"rising_action","label":"Rising Action","guidance":"Complications accumulate on the way up."},
            {"id":"climax","label":"Climax","guidance":"The apex; the protagonist's fortune turns."},
            {"id":"falling_action","label":"Falling Action","guidance":"Consequences unwind toward order or ruin."},
            {"id":"denouement","label":"Denouement","guidance":"Threads tied; the final state revealed."}]}),
        json!({"id":"FIVE_ACT_CLASSIC","label":"Five-Act Structure","lineage":"Shakespearean dramatic arc","legacy_structure":null,"best_for":["STAGE_PLAY","NOVEL"],"beats":[
            {"id":"act1_exposition","label":"Act I · Exposition","guidance":"Stage, stakes and motive established."},
            {"id":"act2_rising","label":"Act II · Rising Action","guidance":"Complication compounds complication."},
            {"id":"act3_climax","label":"Act III · Climax","guidance":"The hinge of fortune, high on the dramatic line."},
            {"id":"act4_falling","label":"Act IV · Falling Action","guidance":"Reversals work themselves out."},
            {"id":"act5_catastrophe","label":"Act V · Catastrophe / Denouement","guidance":"The end-state, tragic or restored."}]}),
        json!({"id":"KISHOTENKETSU","label":"Kishōtenketsu (Four-Act)","lineage":"East-Asian four-part structure","legacy_structure":null,"best_for":["NOVEL","SHORT_STORY","LITERARY"],"beats":[
            {"id":"ki","label":"Ki · Introduction","guidance":"Establish people and situation without conflict."},
            {"id":"sho","label":"Shō · Development","guidance":"Deepen the situation along its own grain."},
            {"id":"ten","label":"Ten · Twist","guidance":"A recontextualizing turn the reader didn't request."},
            {"id":"ketsu","label":"Ketsu · Reconciliation","guidance":"Harmony found that contains the twist."}]}),
        json!({"id":"IN_MEDIAS_RES","label":"In Medias Res","lineage":"Horatian opening","legacy_structure":null,"best_for":["NOVEL","THRILLER","SHORT_STORY"],"beats":[
            {"id":"cold_open","label":"Cold Open","guidance":"Start inside the action's most dangerous hour."},
            {"id":"rewind_exposition","label":"Rewind","guidance":"Step back; how did it come to this?"},
            {"id":"complication_rise","label":"Rising Complication","guidance":"Catch up to the opening moment and pass it."},
            {"id":"second_peak","label":"Second Peak","guidance":"A new summit beyond the opening scene."},
            {"id":"final_confrontation","label":"Final Confrontation","guidance":"The true endgame of the matter."}]}),
        json!({"id":"ROMANCE_BEATS","label":"Romance Beat Sheet","lineage":"Genre consensus (Romancing the Beat school)","legacy_structure":null,"best_for":["NOVEL","NOVELLA"],"beats":[
            {"id":"ordinary_distance","label":"Ordinary Distance","guidance":"Both leads' lives and the wall between them."},
            {"id":"meet_cute","label":"Meet-Cute","guidance":"The first spark, misfired or real."},
            {"id":"forced_proximity","label":"Forced Proximity","guidance":"Plot forbids retreat from each other."},
            {"id":"first_spark","label":"First Spark","guidance":"Attraction admits itself privately."},
            {"id":"deepening_bond","label":"Deepening Bond","guidance":"Vulnerability exchanged; the reader ships."},
            {"id":"internal_conflict","label":"Internal Conflict Surfaces","guidance":"The wound that makes love dangerous."},
            {"id":"breakup","label":"The Break-Up","guidance":"The lie/fear wins; the bond breaks."},
            {"id":"grand_gesture","label":"Grand Gesture","guidance":"Change proven in action, not words."},
            {"id":"declaration","label":"Declaration","guidance":"Both choose, plainly and out loud."},
            {"id":"hea_resolution","label":"HEA / HFN","guidance":"Happily ever after — or for now — earned."}]}),
        json!({"id":"MYSTERY_ARCHITECTURE","label":"Mystery Architecture","lineage":"Detective-fiction craft consensus","legacy_structure":null,"best_for":["NOVEL","COZY_MYSTERY"],"beats":[
            {"id":"the_crime","label":"The Crime (Offstage)","guidance":"The hidden event that created the puzzle."},
            {"id":"discovery","label":"Discovery","guidance":"The body/loss surfaces; the question posed."},
            {"id":"engagement","label":"Detective Engages","guidance":"The sleuth takes the case; rules of the world shown."},
            {"id":"first_interviews","label":"First Round of Interviews","guidance":"Suspect map drawn; alibis logged."},
            {"id":"red_herring_bloom","label":"Red Herrings Bloom","guidance":"False trails honestly planted."},
            {"id":"escalation","label":"Escalation","guidance":"A second event raises the cost of ignorance."},
            {"id":"false_solution","label":"False Solution Collapses","guidance":"The obvious answer fails on a planted fact."},
            {"id":"clue_synthesis","label":"Clue Synthesis","guidance":"A detected pattern reorders every earlier scene."},
            {"id":"reveal","label":"Reveal & Confrontation","guidance":"The howdunit/whodunit stated and tested."},
            {"id":"aftermath","label":"Explanation & Aftermath","guidance":"Justice (or its absence) and the world after."}]}),
        json!({"id":"THRILLER_ENGINE","label":"Thriller Engine","lineage":"Thriller craft consensus","legacy_structure":null,"best_for":["NOVEL","SERIALIZED_WEBNOVEL"],"beats":[
            {"id":"hook_threat","label":"Hook Threat","guidance":"The menace shows its teeth early."},
            {"id":"mission_declared","label":"Mission Declared","guidance":"The hero accepts an impossible job."},
            {"id":"first_assault","label":"First Assault","guidance":"The enemy answers; the plan meets reality."},
            {"id":"investigation_prep","label":"Investigation & Prep","guidance":"Knowledge and tools gathered under clock."},
            {"id":"enemy_advances","label":"Enemy Advances","guidance":"A protected thing is taken or burned."},
            {"id":"the_trap","label":"The Trap","guidance":"The hero walks into the enemy's design."},
            {"id":"lowest_ebb","label":"Lowest Ebb","guidance":"Cornered, outgunned, alone."},
            {"id":"counterstrike","label":"Counterstrike","guidance":"The turn: use the enemy's certainty against them."},
            {"id":"neutralization","label":"Neutralization","guidance":"The threat dismantled; the world recalibrated."}]}),
        json!({"id":"COMING_OF_AGE","label":"Coming-of-Age Arc","lineage":"Bildungsroman","legacy_structure":null,"best_for":["MIDDLE_GRADE_NOVEL","YOUNG_ADULT_NOVEL","MEMOIR"],"beats":[
            {"id":"ordinary_self","label":"Ordinary Self","guidance":"The child's world and its comfortable lie."},
            {"id":"the_fracture","label":"The Fracture","guidance":"A loss or truth cracks the known world."},
            {"id":"first_belonging","label":"First Attempt at Belonging","guidance":"Trying on new tribes and masks."},
            {"id":"mentor_bond","label":"Mentor / Peer Bond","guidance":"Someone sees the protagonist clearly."},
            {"id":"transgression","label":"Transgression","guidance":"A line crossed; adulthood's price tasted."},
            {"id":"cost_of_growth","label":"Cost of Growth","guidance":"The old self mourns; consequences owned."},
            {"id":"reconciliation","label":"Reconciliation","guidance":"Peace with family/self/world renegotiated."},
            {"id":"new_self","label":"New Self","guidance":"Identity rebuilt; the child stands."}]}),
        json!({"id":"NONFICTION_PROBLEM_SOLUTION","label":"Non-Fiction: Problem → Solution","lineage":"Practical non-fiction consensus","legacy_structure":null,"best_for":["SELF_HELP","BUSINESS_BOOK","HOW_TO_GUIDE"],"beats":[
            {"id":"pain_hook","label":"Pain Hook","guidance":"The reader's problem stated in their own voice."},
            {"id":"stakes_promise","label":"Stakes & Promise","guidance":"What it costs to stay stuck; what the book delivers."},
            {"id":"root_cause","label":"Root Cause","guidance":"Why the problem persists — the real mechanism."},
            {"id":"the_framework","label":"The Framework","guidance":"The named system that will fix it."},
            {"id":"applied_steps","label":"Applied Steps","guidance":"The framework run on real cases, step by step."},
            {"id":"objections_obstacles","label":"Objections & Obstacles","guidance":"Failure modes answered honestly."},
            {"id":"transformation_next","label":"Transformation & Next Steps","guidance":"The after-state and the reader's first move."}]}),
        json!({"id":"MEMOIR_ARC","label":"Memoir Arc","lineage":"Memoir craft consensus","legacy_structure":null,"best_for":["MEMOIR","ESSAY_COLLECTION"],"beats":[
            {"id":"snapshot_then","label":"Snapshot of Then","guidance":"The life before, told with the end in mind."},
            {"id":"inciting_wound","label":"Inciting Wound","guidance":"The event that sets the question of a life."},
            {"id":"leaving_known","label":"Leaving the Known","guidance":"The attempt to outrun or answer the wound."},
            {"id":"the_descent","label":"The Descent","guidance":"Patterns repeat; costs compound."},
            {"id":"reckoning","label":"Reckoning","guidance":"The truth faced without anesthetic."},
            {"id":"turning_point","label":"Turning Point","guidance":"A choice that rewrites the pattern."},
            {"id":"integration","label":"Integration","guidance":"The new way of living, tested by ordinary days."},
            {"id":"return_reflection","label":"Return & Reflection","guidance":"The narrator's present voice blesses or buries the past."}]}),
        json!({"id":"DUAL_TIMELINE","label":"Dual Timeline","lineage":"Historical/contemporary braided form","legacy_structure":null,"best_for":["NOVEL","NARRATIVE_HISTORY"],"beats":[
            {"id":"present_hook","label":"Present Hook","guidance":"A modern mystery or fracture opens the now-line."},
            {"id":"past_seed","label":"Past Seed","guidance":"The then-line plants the origin."},
            {"id":"present_complication","label":"Present Complication","guidance":"The now-line deepens; clues point back."},
            {"id":"past_escalation","label":"Past Escalation","guidance":"The then-line stakes rise."},
            {"id":"mirror_point","label":"Mirror Point","guidance":"The timelines rhyme; characters echo across decades."},
            {"id":"present_crisis","label":"Present Crisis","guidance":"The now-line reaches its own edge."},
            {"id":"past_revelation","label":"Past Revelation","guidance":"The then-line discloses what explains the now."},
            {"id":"converged_resolution","label":"Converged Resolution","guidance":"Both lines resolve into one truth."}]}),
        json!({"id":"EPISODIC_SERIAL","label":"Episodic Serial Loop","lineage":"Pulp/serial craft consensus","legacy_structure":null,"best_for":["SERIALIZED_WEBNOVEL","FLASH_COLLECTION"],"beats":[
            {"id":"cold_hook","label":"Cold Hook","guidance":"Open each episode on motion or trouble."},
            {"id":"episode_goal","label":"Episode Goal","guidance":"A small want achievable within the episode."},
            {"id":"complication","label":"Complication","guidance":"The want meets friction."},
            {"id":"reversal","label":"Reversal","guidance":"The plan breaks; improvisation begins."},
            {"id":"mini_climax","label":"Mini-Climax","guidance":"The episode's answer, earned."},
            {"id":"serial_thread","label":"Serial Thread Beat","guidance":"Advance the long arc one notch."},
            {"id":"cliffhanger","label":"Cliffhanger","guidance":"End on the question that buys the next click."},
            {"id":"loop_reset","label":"Loop Reset","guidance":"Status quo + damage; the loop can turn again."}]}),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Storefront pricing constants (Amazon KDP planning approximation).
// NOTE: rates drift — verify current figures on kdp.amazon.com before filing
// prices. These constants exist so the offline app can project royalties.
// ─────────────────────────────────────────────────────────────────────────────

pub fn pricing() -> Value {
    json!({
        "marketplace": "Amazon KDP (US, USD) — planning approximation; verify current rates on kdp.amazon.com",
        "ebook": {
            "rate_70_min_price": 2.99,
            "rate_70_max_price": 9.99,
            "rate_70": 0.70,
            "rate_other": 0.35,
            "delivery_usd_per_mb": 0.15,
            "note": "70% tier minus delivery ($0.15/MB); 35% tier has no delivery fee"
        },
        "paperback": {
            "royalty_rate": 0.60,
            "print_fixed_usd": 0.85,
            "print_per_page_usd": 0.012,
            "note": "US black-ink white-paper model: 60% minus print cost"
        },
        "hardcover": {
            "royalty_rate": 0.60,
            "print_fixed_usd": 5.65,
            "print_per_page_usd": 0.0122,
            "note": "US standard hardcover model"
        },
        "audiobook_acx": {
            "exclusive": 0.40,
            "non_exclusive": 0.25,
            "note": "ACX royalty share (sold via Audible, not KDP itself)"
        },
        "words_per_print_page": 250,
        "ebook_mb_per_10k_words": 0.35
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Lookups — pure scans over the frozen tables.
// ─────────────────────────────────────────────────────────────────────────────

pub fn lookup(list: &[Value], id: &str) -> Option<Value> {
    list.iter().find(|e| e["id"] == json!(id)).cloned()
}

pub fn format_by_id(id: &str) -> Option<Value> {
    lookup(&formats(), id)
}
pub fn persona_by_id(id: &str) -> Option<Value> {
    lookup(&personas(), id)
}
pub fn arc_by_id(id: &str) -> Option<Value> {
    lookup(&arcs(), id)
}

/// Full or sliced catalog payload for `craft_catalog`.
pub fn catalog_json(kind: &str) -> Result<Value, String> {
    match kind {
        "formats" => Ok(json!({"kind":"formats","entries":formats(),"count":formats().len()})),
        "personas" => Ok(json!({"kind":"personas","entries":personas(),"count":personas().len()})),
        "arcs" => Ok(json!({"kind":"arcs","entries":arcs(),"count":arcs().len()})),
        "pricing" => Ok(json!({"kind":"pricing","entries":[pricing()],"count":1})),
        // v1.4: the SFB-2.0 retention blueprint rides the same surface.
        "retention" => {
            let bp = crate::craft::retention::blueprint_json();
            Ok(json!({"kind":"retention","entries":[bp],"count":1,"blueprint":bp}))
        }
        "all" => Ok(json!({
            "kind": "all",
            "formats": formats(),
            "personas": personas(),
            "arcs": arcs(),
            "pricing": pricing(),
            "retention": crate::craft::retention::blueprint_json(),
            "counts": {"formats": formats().len(), "personas": personas().len(), "arcs": arcs().len()},
            "offline": true
        })),
        other => Err(format!(
            "unknown catalog kind `{other}` — use formats|personas|arcs|pricing|retention|all"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique_within_kind() {
        for (name, list) in [
            ("formats", formats()),
            ("personas", personas()),
            ("arcs", arcs()),
        ] {
            let mut ids: Vec<&str> = list.iter().map(|e| e["id"].as_str().unwrap()).collect();
            let total = ids.len();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), total, "duplicate ids in {name} catalog");
        }
    }

    #[test]
    fn catalogs_have_promised_sizes() {
        assert_eq!(formats().len(), 25);
        assert_eq!(personas().len(), 37);
        assert_eq!(arcs().len(), 18);
    }

    #[test]
    fn every_persona_is_llm_directive_complete() {
        for p in personas() {
            let d = p["directive"].as_str().unwrap_or_default();
            assert!(d.len() > 40, "persona {} needs a real directive", p["id"]);
            assert!(p["pov"].as_str().is_some() && p["rhythm"].as_str().is_some());
        }
    }

    #[test]
    fn every_arc_has_beats_and_legal_legacy_mapping() {
        const LEGAL: [&str; 4] = ["SAVE_THE_CAT", "THREE_ACT", "FICHTEAN_CURVE", "HEROS_JOURNEY"];
        for a in arcs() {
            let beats = a["beats"].as_array().expect("beats array");
            assert!(beats.len() >= 4, "arc {} needs >= 4 beats", a["id"]);
            if let Some(leg) = a["legacy_structure"].as_str() {
                assert!(LEGAL.contains(&leg), "arc {} illegal legacy {leg}", a["id"]);
            }
        }
    }

    #[test]
    fn lookups_round_trip() {
        assert!(format_by_id("NOVEL").is_some());
        assert!(format_by_id("NO_SUCH").is_none());
        assert!(persona_by_id("EDGE_THRILLER").is_some());
        assert!(arc_by_id("KISHOTENKETSU").is_some());
        assert_eq!(
            arc_by_id("SAVE_THE_CAT").unwrap()["legacy_structure"],
            "SAVE_THE_CAT"
        );
        assert!(arc_by_id("STORY_CIRCLE").unwrap()["legacy_structure"].is_null());
    }

    #[test]
    fn catalog_json_slices() {
        assert_eq!(catalog_json("all").unwrap()["counts"]["arcs"], 18);
        assert!(catalog_json("pricing").is_ok());
        assert!(catalog_json("nope").is_err());
    }
}
