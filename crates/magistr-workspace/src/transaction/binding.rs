use super::{ExpectedState, FormatMode, PreparedTransaction, VerificationStatus};
use crate::file_content::FileContent;

/// Length-framed raw bytes keep sealing memory proportional to the bounded
/// hydrated plan. A JSON Value per source byte would amplify memory badly.
pub(super) fn bytes(prepared: &PreparedTransaction) -> Vec<u8> {
    let mut output = b"magistr-final-plan-v1\0".to_vec();
    output.push(match prepared.request.format {
        FormatMode::Enabled => 1,
        FormatMode::Disabled => 0,
    });
    number(&mut output, prepared.request.undo_of.len());
    for seq in &prepared.request.undo_of {
        output.extend_from_slice(&seq.to_le_bytes());
    }
    number(&mut output, prepared.preview.len());
    for (index, file) in prepared.preview.iter().enumerate() {
        field(&mut output, prepared.request.changes[index].path.as_bytes());
        field(
            &mut output,
            prepared.paths[index].as_os_str().as_encoded_bytes(),
        );
        field(&mut output, file.path.as_bytes());
        match &prepared.request.changes[index].expected {
            ExpectedState::Missing => output.push(0),
            ExpectedState::Hash(hash) => {
                output.push(1);
                field(&mut output, hash.as_bytes());
            }
        }
        content(&mut output, &file.before);
        content(&mut output, &file.after);
        permissions(&mut output, prepared.snapshots[index].permissions.as_ref());
        number(&mut output, file.diagnostics.len());
        for diagnostic in &file.diagnostics {
            field(&mut output, diagnostic.as_bytes());
        }
        field(&mut output, file.formatting.as_bytes());
        output.push(match file.verification {
            VerificationStatus::Verified => 0,
            VerificationStatus::ExistingDiagnostics => 1,
            VerificationStatus::Unsupported => 2,
            VerificationStatus::Binary => 3,
            VerificationStatus::Deletion => 4,
            VerificationStatus::Unchanged => 5,
        });
    }
    number(&mut output, prepared.baselines.len());
    for (path, admitted, snapshot) in &prepared.baselines {
        field(&mut output, path.as_bytes());
        field(&mut output, admitted.as_os_str().as_encoded_bytes());
        content(&mut output, &snapshot.content);
        permissions(&mut output, snapshot.permissions.as_ref());
    }
    output
}
fn number(output: &mut Vec<u8>, value: usize) {
    output.extend_from_slice(&(value as u64).to_le_bytes());
}
fn field(output: &mut Vec<u8>, bytes: &[u8]) {
    number(output, bytes.len());
    output.extend_from_slice(bytes);
}
fn content(output: &mut Vec<u8>, content: &Option<FileContent>) {
    match content {
        None => output.push(0),
        Some(content) => {
            output.push(1);
            field(output, content.as_bytes());
        }
    }
}
fn permissions(output: &mut Vec<u8>, permissions: Option<&std::fs::Permissions>) {
    let Some(permissions) = permissions else {
        output.push(0);
        return;
    };
    output.push(1);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        output.extend_from_slice(&permissions.mode().to_le_bytes());
    }
    #[cfg(not(unix))]
    {
        output.push(u8::from(permissions.readonly()));
    }
}
