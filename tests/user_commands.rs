//! Cycle 10, T5: slash commands from `~/.colctl/commands`. Named after AC10-NN / EC10-NN.

mod support;

use std::fs;
use std::path::Path;
use support::Env;

fn machine() -> Env {
    let env = Env::new();
    env.native();
    env.kilo();
    env.codex();
    env.kilo_codex();
    env.agents_dir();
    env.ok(&["init"]);
    env
}

const STANDUP: &str = "---\ndescription: Write today's standup\n---\nSummarise what I did yesterday for $ARGUMENTS.\n";

fn source_command(env: &Env, name: &str, text: &str) {
    let dir = env.source.join("commands");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{name}.md")), text).unwrap();
}

fn state(env: &Env, harness: &str, name: &str) -> Option<String> {
    env.item_state(harness, "command", name)
}

fn skill_text(env: &Env, name: &str) -> Option<String> {
    fs::read_to_string(env.agents_dir().join("skills").join(name).join("SKILL.md")).ok()
}

fn link_of(p: &Path) -> Option<std::path::PathBuf> {
    fs::read_link(p).ok()
}

#[test]
fn ac10_40_a_command_reaches_every_profile() {
    let env = machine();
    source_command(&env, "standup", STANDUP);
    env.ok(&["sync"]);
    let target = env.source.join("commands/standup.md");
    for profile in [env.native(), env.kilo()] {
        assert_eq!(
            link_of(&profile.join("commands/standup.md")).as_deref(),
            Some(target.as_path()),
            "{}",
            profile.display()
        );
    }
    let skill = skill_text(&env, "standup").expect("a skill for Codex");
    assert!(
        skill.starts_with("---\nname: standup\ndescription: Write today's standup\n---\n"),
        "{skill}"
    );
    assert!(
        skill.contains("Summarise what I did yesterday for the user's request."),
        "{skill}"
    );
    for h in ["claude", "kilo-claude", "agents"] {
        assert_eq!(state(&env, h, "standup").as_deref(), Some("ok"), "{h}");
    }
    let (code, _, _) = env.code(&["status"]);
    assert_eq!(code, 0);
    // A second sync changes nothing.
    let out = env.ok(&["sync"]);
    assert!(
        out.contains("already in sync") || out.contains("nothing to change"),
        "{out}"
    );
}

#[test]
fn ac10_41_add_command_and_rm_command() {
    let env = machine();
    let file = env.home.join("standup.md");
    fs::write(&file, STANDUP).unwrap();
    let commits = env.commits().len();
    env.ok(&["add", "command", file.to_str().unwrap()]);
    assert_eq!(
        fs::read_to_string(env.source.join("commands/standup.md")).unwrap(),
        STANDUP
    );
    assert_eq!(env.commits().len(), commits + 1);
    assert!(env.native().join("commands/standup.md").exists());
    assert!(skill_text(&env, "standup").is_some());

    env.ok(&["rm", "command", "standup", "--yes"]);
    assert!(!env.source.join("commands/standup.md").exists());
    assert!(fs::symlink_metadata(env.native().join("commands/standup.md")).is_err());
    assert!(!env.agents_dir().join("skills/standup").exists());
    assert_eq!(state(&env, "agents", "standup"), None);
    assert_eq!(state(&env, "agents", "standup"), None);
}

#[test]
fn ac10_42_adopt_a_command() {
    let env = machine();
    let file = env.native().join("commands/notes.md");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "Take notes on $ARGUMENTS\n").unwrap();
    assert_eq!(state(&env, "claude", "notes").as_deref(), Some("foreign"));
    env.ok(&["adopt", "claude", "commands/notes.md"]);
    let moved = env.source.join("commands/notes.md");
    assert_eq!(
        fs::read_to_string(&moved).unwrap(),
        "Take notes on $ARGUMENTS\n"
    );
    assert_eq!(link_of(&file).as_deref(), Some(moved.as_path()));
    assert!(env.kilo().join("commands/notes.md").exists());
}

#[test]
fn ec10_18_no_frontmatter_or_empty_body() {
    let env = machine();
    source_command(
        &env,
        "plain",
        "\n\nReview the diff carefully\nand say what's wrong.\n",
    );
    source_command(&env, "empty", "");
    env.ok(&["sync"]);
    let plain = skill_text(&env, "plain").unwrap();
    assert!(
        plain.contains("description: Review the diff carefully\n"),
        "{plain}"
    );
    let empty = skill_text(&env, "empty").unwrap();
    assert!(empty.contains("description: Command empty\n"), "{empty}");
    assert!(env.native().join("commands/empty.md").exists());
}

#[test]
fn ec10_19_a_command_named_like_a_skill() {
    let env = machine();
    env.source_skill("review");
    source_command(&env, "review", STANDUP);
    let (code, _, _) = env.code(&["sync"]);
    assert_ne!(code, 0);
    // Claude gets both; ~/.agents keeps the skill and says why the command isn't there.
    assert!(env.native().join("commands/review.md").exists());
    assert!(env.native().join("skills/review").exists());
    assert_eq!(state(&env, "agents", "review").as_deref(), Some("invalid"));
    let text = env.expect(1, &["status"]);
    assert!(text.contains("a skill is also called review"), "{text}");
}

#[test]
fn ac10_40_an_edited_generated_skill_is_left_alone() {
    let env = machine();
    source_command(&env, "standup", STANDUP);
    env.ok(&["sync"]);
    let path = env.agents_dir().join("skills/standup/SKILL.md");
    fs::write(&path, "mine").unwrap();
    assert_eq!(state(&env, "agents", "standup").as_deref(), Some("edited"));
    env.code(&["sync"]);
    assert_eq!(fs::read_to_string(&path).unwrap(), "mine");
}

#[test]
fn ac10_40_a_changed_command_rewrites_the_skill() {
    let env = machine();
    source_command(&env, "standup", STANDUP);
    env.ok(&["sync"]);
    source_command(&env, "standup", &STANDUP.replace("today's", "the"));
    env.ok(&["sync"]);
    assert!(
        skill_text(&env, "standup")
            .unwrap()
            .contains("Write the standup")
    );
}
