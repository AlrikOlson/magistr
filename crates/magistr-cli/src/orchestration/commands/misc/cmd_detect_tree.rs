use crate::detect;
use crate::error::ResultExt;
use crate::output::OutputRenderer;
use color_eyre::eyre;

pub fn cmd_detect_tree(dir: &std::path::Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let tree = detect::build_project_tree(dir).suggest()?;

    if renderer.is_machine_readable() {
        let data = serde_json::to_value(&tree)?;
        renderer
            .json(&mut std::io::stdout(), &data)
            .map_err(|e| crate::error::MagistrError::io(dir, e))?;
    } else {
        print_tree_view(&tree);
    }

    Ok(())
}

fn print_tree_view(tree: &magistr_core::types::ProjectTree) {
    use magistr_core::types::Topology;

    if tree.is_empty() {
        eprintln!("No projects detected.");
        return;
    }

    // Header
    let topology_str = match tree.topology {
        Topology::SingleProject => "Single Project",
        Topology::FlatMonorepo => "Flat Monorepo",
        Topology::NestedFullstack => "Nested Fullstack",
        Topology::Workspace => "Workspace",
    };
    let workspace_str = tree.workspace.as_str();

    eprintln!(
        "Project Tree ({topology_str}, workspace: {workspace_str}) — {} project(s)",
        tree.len()
    );
    eprintln!();

    // Find root-level projects (no parent)
    let roots: Vec<usize> = tree
        .projects
        .iter()
        .enumerate()
        .filter(|(_, p)| p.parent.is_none())
        .map(|(i, _)| i)
        .collect();

    for (i, &root_idx) in roots.iter().enumerate() {
        let is_last = i == roots.len() - 1;
        print_tree_node(tree, root_idx, "", is_last);
    }

    // Print edges if any
    if !tree.edges.is_empty() {
        eprintln!();
        eprintln!("Dependencies:");
        for edge in &tree.edges {
            let from = &tree.projects[edge.from];
            let to = &tree.projects[edge.to];
            let from_name = project_label(from);
            let to_name = project_label(to);
            let kind = match edge.kind {
                magistr_core::types::EdgeKind::CargoPathDep => "cargo path",
                magistr_core::types::EdgeKind::NpmWorkspaceRef => "workspace ref",
                magistr_core::types::EdgeKind::CsprojRef => "project ref",
                magistr_core::types::EdgeKind::SharedDir => "shared dir",
            };
            eprintln!("  {from_name} → {to_name} ({kind})");
        }
    }
}

fn print_tree_node(
    tree: &magistr_core::types::ProjectTree,
    idx: usize,
    prefix: &str,
    is_last: bool,
) {
    let project = &tree.projects[idx];
    let connector = if is_last { "└── " } else { "├── " };
    let label = project_label(project);

    eprintln!("{prefix}{connector}{label}");

    let child_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
    for (i, &child_idx) in project.children.iter().enumerate() {
        let child_is_last = i == project.children.len() - 1;
        print_tree_node(tree, child_idx, &child_prefix, child_is_last);
    }
}

fn project_label(project: &magistr_core::types::DetectedProject) -> String {
    let path = if project.path.as_os_str().is_empty() {
        ".".to_string()
    } else {
        project.path.display().to_string()
    };

    let lang = project.language.as_str();
    let role = project.role.as_str();

    let framework = project
        .framework
        .map(|f| format!("/{}", f.as_str()))
        .unwrap_or_default();

    let name = project
        .metadata
        .name
        .as_deref()
        .map(|n| format!(" ({n})"))
        .unwrap_or_default();

    format!("{path}{name} [{lang}{framework}] {role}")
}
