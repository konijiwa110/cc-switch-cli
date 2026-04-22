#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
ENV_FILE="${CCSWITCH_ENV_FILE:-${REPO_ROOT}/.env}"
WORK_DIR="${CCSWITCH_WORK_DIR:-${REPO_ROOT}/.tmp/distribute-release}"
ASKPASS_SCRIPT="${WORK_DIR}/askpass.sh"
RELEASE_WORKFLOW="${CCSWITCH_RELEASE_WORKFLOW:-release.yml}"
BUILD_BRANCH="${CCSWITCH_BUILD_BRANCH:-main}"
GH_POLL_SECONDS="${CCSWITCH_GH_POLL_SECONDS:-20}"
GH_TIMEOUT_SECONDS="${CCSWITCH_GH_TIMEOUT_SECONDS:-3600}"
DRY_RUN=0

info() {
  printf '  \033[1;32minfo\033[0m: %s\n' "$*"
}

warn() {
  printf '  \033[1;33mwarn\033[0m: %s\n' "$*" >&2
}

err() {
  printf '  \033[1;31merror\033[0m: %s\n' "$*" >&2
}

die() {
  err "$*"
  exit 1
}

usage() {
  cat <<'EOF'
Usage: scripts/distribute_release.sh [--dry-run] [--env-file PATH]

Flow:
1. Push current HEAD to the configured GitHub build repo default branch
2. Push a temporary tag and wait for the Release workflow
3. Download linux-x64-musl and linux-arm64-musl artifacts
4. Install local machine first
5. Install 100.64.0.13, then 100.64.0.2:2323, then 100.64.0.10
EOF
}

run_cmd() {
  if ((DRY_RUN)); then
    printf '  [dry-run]'
    printf ' %q' "$@"
    printf '\n'
    return 0
  fi
  "$@"
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "Required command not found: $1"
}

require_var() {
  local name="$1"
  [[ -n "${!name:-}" ]] || die "Missing required config: ${name}"
}

load_env() {
  [[ -f "${ENV_FILE}" ]] || die "Env file not found: ${ENV_FILE}"
  set -a
  # shellcheck disable=SC1090
  source "${ENV_FILE}"
  set +a
}

init_defaults() {
  CCSWITCH_REMOTE_13_HOST="${CCSWITCH_REMOTE_13_HOST:-100.64.0.13}"
  CCSWITCH_REMOTE_13_PORT="${CCSWITCH_REMOTE_13_PORT:-22}"
  CCSWITCH_REMOTE_13_USER="${CCSWITCH_REMOTE_13_USER:-jasper}"
  CCSWITCH_REMOTE_13_SUDO_PASSWORD="${CCSWITCH_REMOTE_13_SUDO_PASSWORD:-${CCSWITCH_REMOTE_13_PASSWORD:-}}"
  CCSWITCH_REMOTE_13_TARGET="${CCSWITCH_REMOTE_13_TARGET:-}"

  CCSWITCH_UGREEN_HOST="${CCSWITCH_UGREEN_HOST:-100.64.0.2}"
  CCSWITCH_UGREEN_PORT="${CCSWITCH_UGREEN_PORT:-2323}"
  CCSWITCH_UGREEN_USER="${CCSWITCH_UGREEN_USER:-jasper}"
  CCSWITCH_UGREEN_SUDO_PASSWORD="${CCSWITCH_UGREEN_SUDO_PASSWORD:-${CCSWITCH_UGREEN_PASSWORD:-}}"
  CCSWITCH_UGREEN_TARGET="${CCSWITCH_UGREEN_TARGET:-}"

  CCSWITCH_PANTHERX2_HOST="${CCSWITCH_PANTHERX2_HOST:-100.64.0.10}"
  CCSWITCH_PANTHERX2_PORT="${CCSWITCH_PANTHERX2_PORT:-22}"
  CCSWITCH_PANTHERX2_USER="${CCSWITCH_PANTHERX2_USER:-root}"
  CCSWITCH_PANTHERX2_SUDO_PASSWORD="${CCSWITCH_PANTHERX2_SUDO_PASSWORD:-${CCSWITCH_PANTHERX2_PASSWORD:-}}"
  CCSWITCH_PANTHERX2_TARGET="${CCSWITCH_PANTHERX2_TARGET:-}"
}

ensure_prereqs() {
  require_cmd gh
  require_cmd git
  require_cmd curl
  require_cmd unzip
  require_cmd ssh
  require_cmd scp
  require_cmd setsid
  require_cmd sha256sum
  require_cmd base64
  require_cmd python3
  gh auth status >/dev/null 2>&1 || die "GitHub CLI is not authenticated"
}

assert_clean_worktree() {
  local status
  status="$(git -C "${REPO_ROOT}" status --short)"
  if [[ -n "${status}" && "${CCSWITCH_ALLOW_DIRTY:-0}" != "1" ]]; then
    die "Git worktree is dirty. Commit first, or set CCSWITCH_ALLOW_DIRTY=1 if you really want to build from current HEAD only."
  fi
}

repo_version() {
  sed -n 's/^version = "\(.*\)"/\1/p' "${REPO_ROOT}/src-tauri/Cargo.toml" | head -n 1
}

create_askpass_script() {
  mkdir -p "${WORK_DIR}"
  cat > "${ASKPASS_SCRIPT}" <<'EOF'
#!/usr/bin/env bash
printf '%s' "${CCSWITCH_ASKPASS_SECRET:?missing askpass secret}"
EOF
  chmod 700 "${ASKPASS_SCRIPT}"
}

with_askpass() {
  local password="$1"
  shift

  if ((DRY_RUN)); then
    printf '  [dry-run]'
    printf ' %q' "$@"
    printf '\n'
    return 0
  fi

  env \
    DISPLAY="cc-switch" \
    SSH_ASKPASS="${ASKPASS_SCRIPT}" \
    SSH_ASKPASS_REQUIRE="force" \
    CCSWITCH_ASKPASS_SECRET="${password}" \
    setsid "$@"
}

git_auth_header() {
  local token
  token="$(gh auth token)"
  printf 'AUTHORIZATION: basic %s' "$(printf 'x-access-token:%s' "${token}" | base64 -w0)"
}

wait_for_release_run() {
  local build_repo="$1"
  local tag="$2"
  local deadline run_id

  deadline=$(( $(date +%s) + GH_TIMEOUT_SECONDS ))
  while (( $(date +%s) < deadline )); do
    run_id="$(
      gh run list \
        -R "${build_repo}" \
        --workflow "${RELEASE_WORKFLOW}" \
        --event push \
        --limit 20 \
        --json databaseId,headBranch \
        --jq ".[] | select(.headBranch == \"${tag}\") | .databaseId" \
        | head -n 1
    )"
    if [[ -n "${run_id}" ]]; then
      printf '%s\n' "${run_id}"
      return 0
    fi
    sleep "${GH_POLL_SECONDS}"
  done

  die "Timed out waiting for release workflow run for tag ${tag}"
}

wait_for_job_success() {
  local build_repo="$1"
  local run_id="$2"
  local job_name="$3"
  local deadline status conclusion

  deadline=$(( $(date +%s) + GH_TIMEOUT_SECONDS ))
  while (( $(date +%s) < deadline )); do
    status="$(
      gh run view "${run_id}" \
        -R "${build_repo}" \
        --json jobs \
        --jq ".jobs[] | select(.name == \"${job_name}\") | .status" \
        | head -n 1
    )"
    conclusion="$(
      gh run view "${run_id}" \
        -R "${build_repo}" \
        --json jobs \
        --jq ".jobs[] | select(.name == \"${job_name}\") | .conclusion" \
        | head -n 1
    )"

    if [[ "${status}" == "completed" ]]; then
      [[ "${conclusion}" == "success" ]] || die "${job_name} failed with conclusion=${conclusion}"
      return 0
    fi

    sleep "${GH_POLL_SECONDS}"
  done

  die "Timed out waiting for ${job_name}"
}

artifact_download_url() {
  local build_repo="$1"
  local run_id="$2"
  local artifact_name="$3"

  gh api "repos/${build_repo}/actions/runs/${run_id}/artifacts" \
    --jq ".artifacts[] | select(.name == \"${artifact_name}\") | .archive_download_url" \
    | head -n 1
}

download_artifact() {
  local url="$1"
  local output_zip="$2"

  if ((DRY_RUN)); then
    info "Would download artifact to ${output_zip}"
    return 0
  fi

  curl -fL --retry 3 --retry-delay 2 \
    -H "Authorization: Bearer $(gh auth token)" \
    -H "Accept: application/vnd.github+json" \
    -o "${output_zip}" \
    "${url}"
}

local_install_target() {
  if [[ -n "${CCSWITCH_LOCAL_TARGET:-}" ]]; then
    printf '%s\n' "${CCSWITCH_LOCAL_TARGET}"
    return 0
  fi

  local resolved
  resolved="$(command -v cc-switch || true)"
  if [[ -n "${resolved}" ]]; then
    printf '%s\n' "${resolved}"
  else
    printf '/usr/local/bin/cc-switch\n'
  fi
}

backup_existing_binary() {
  local target="$1"
  local suffix="$2"
  local backup_dir backup_path

  backup_dir="${HOME}/.local/state/cc-switch-backups"
  mkdir -p "${backup_dir}"
  if [[ -f "${target}" ]]; then
    backup_path="${backup_dir}/$(basename "${target}").${suffix}.$(date +%Y%m%d-%H%M%S)"
    cp "${target}" "${backup_path}"
    printf '%s\n' "${backup_path}"
  fi
}

install_local_binary() {
  local source_binary="$1"
  local target backup_path

  target="$(local_install_target)"
  info "Installing local machine -> ${target}"
  backup_path="$(backup_existing_binary "${target}" "deploy")" || true
  if [[ -n "${backup_path:-}" ]]; then
    info "Local backup -> ${backup_path}"
  fi

  if ((DRY_RUN)); then
    info "Would install ${source_binary} -> ${target}"
    return 0
  fi

  if [[ -w "${target}" || -w "$(dirname "${target}")" ]]; then
    install -m 0755 "${source_binary}" "${target}"
  else
    sudo install -m 0755 "${source_binary}" "${target}"
  fi

  sha256sum "${source_binary}" "${target}"
  "${target}" --version
}

remote_install() {
  local label="$1"
  local host="$2"
  local port="$3"
  local user="$4"
  local password="$5"
  local sudo_password="$6"
  local target_override="$7"
  local source_binary="$8"
  local remote_tmp remote_script

  remote_tmp="/tmp/cc-switch-${label}-$$"
  info "Uploading ${label} -> ${user}@${host}:${port}"
  with_askpass "${password}" \
    scp \
    -P "${port}" \
    -o PreferredAuthentications=password \
    -o PubkeyAuthentication=no \
    -o StrictHostKeyChecking=accept-new \
    "${source_binary}" \
    "${user}@${host}:${remote_tmp}"

  remote_script=$(cat <<'EOF'
set -Eeuo pipefail

src="$1"
sudo_password="$2"
target_override="$3"

if [[ -n "${target_override}" ]]; then
  target="${target_override}"
else
  resolved="$(command -v cc-switch || true)"
  if [[ -n "${resolved}" ]]; then
    target="${resolved}"
  else
    target="/usr/local/bin/cc-switch"
  fi
fi

backup_dir="${HOME}/.local/state/cc-switch-backups"
mkdir -p "${backup_dir}"

echo "target:${target}"
if [[ -f "${target}" ]]; then
  backup_path="${backup_dir}/$(basename "${target}").deploy.$(date +%Y%m%d-%H%M%S)"
  cp "${target}" "${backup_path}"
  echo "backup:${backup_path}"
fi

if [[ -w "${target}" || -w "$(dirname "${target}")" ]]; then
  install -m 0755 "${src}" "${target}"
  echo "install_mode:user"
else
  command -v sudo >/dev/null 2>&1 || {
    echo "sudo is required to install ${target}" >&2
    exit 1
  }
  printf '%s\n' "${sudo_password}" | sudo -S -p '' install -m 0755 "${src}" "${target}"
  echo "install_mode:sudo"
fi

sha256sum "${src}" "${target}"
"${target}" --version
rm -f "${src}"
EOF
)

  with_askpass "${password}" \
    ssh \
    -p "${port}" \
    -o PreferredAuthentications=password \
    -o PubkeyAuthentication=no \
    -o StrictHostKeyChecking=accept-new \
    "${user}@${host}" \
    "bash -s -- $(printf '%q' "${remote_tmp}") $(printf '%q' "${sudo_password}") $(printf '%q' "${target_override}")" \
    <<< "${remote_script}"
}

main() {
  local version short_sha build_repo tag run_id
  local x64_artifact_name arm64_artifact_name
  local x64_zip arm64_zip x64_dir arm64_dir x64_bin arm64_bin

  while (($# > 0)); do
    case "$1" in
      --dry-run)
        DRY_RUN=1
        ;;
      --env-file)
        shift
        [[ $# -gt 0 ]] || die "--env-file requires a path"
        ENV_FILE="$1"
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        die "Unknown argument: $1"
        ;;
    esac
    shift
  done

  load_env
  init_defaults
  ensure_prereqs
  assert_clean_worktree

  require_var CCSWITCH_BUILD_REPO
  require_var CCSWITCH_REMOTE_13_PASSWORD
  require_var CCSWITCH_UGREEN_PASSWORD
  require_var CCSWITCH_PANTHERX2_PASSWORD

  create_askpass_script
  mkdir -p "${WORK_DIR}"

  version="$(repo_version)"
  short_sha="$(git -C "${REPO_ROOT}" rev-parse --short=12 HEAD)"
  build_repo="${CCSWITCH_BUILD_REPO}"
  tag="${CCSWITCH_BUILD_TAG:-v${version}-deploy-$(date +%Y%m%d-%H%M%S)}"

  info "Build repo -> ${build_repo}"
  info "Build branch -> ${BUILD_BRANCH}"
  info "Build tag -> ${tag}"

  if ((DRY_RUN)); then
    info "Would push HEAD ${short_sha} -> ${build_repo}:${BUILD_BRANCH}"
    info "Would push tag ${tag} -> ${build_repo}"
    info "Would wait for Release workflow and install local + 3 remotes in fixed order"
    exit 0
  fi

  gh repo view "${build_repo}" --json nameWithOwner >/dev/null
  gh workflow view "${RELEASE_WORKFLOW}" -R "${build_repo}" >/dev/null

  run_cmd git -C "${REPO_ROOT}" \
    -c "http.https://github.com/.extraheader=$(git_auth_header)" \
    push --force "https://github.com/${build_repo}.git" "HEAD:refs/heads/${BUILD_BRANCH}"

  run_cmd git -C "${REPO_ROOT}" \
    -c "http.https://github.com/.extraheader=$(git_auth_header)" \
    push "https://github.com/${build_repo}.git" "HEAD:refs/tags/${tag}"

  run_id="$(wait_for_release_run "${build_repo}" "${tag}")"
  info "Release run -> https://github.com/${build_repo}/actions/runs/${run_id}"

  wait_for_job_success "${build_repo}" "${run_id}" "Build linux-x64-musl"
  wait_for_job_success "${build_repo}" "${run_id}" "Build linux-arm64-musl"

  x64_artifact_name="cc-switch-cli-linux-x64-musl"
  arm64_artifact_name="cc-switch-cli-linux-arm64-musl"
  x64_zip="${WORK_DIR}/${x64_artifact_name}.zip"
  arm64_zip="${WORK_DIR}/${arm64_artifact_name}.zip"
  x64_dir="${WORK_DIR}/${x64_artifact_name}"
  arm64_dir="${WORK_DIR}/${arm64_artifact_name}"

  download_artifact "$(artifact_download_url "${build_repo}" "${run_id}" "${x64_artifact_name}")" "${x64_zip}"
  download_artifact "$(artifact_download_url "${build_repo}" "${run_id}" "${arm64_artifact_name}")" "${arm64_zip}"

  rm -rf "${x64_dir}" "${arm64_dir}"
  mkdir -p "${x64_dir}" "${arm64_dir}"
  unzip -oq "${x64_zip}" -d "${x64_dir}"
  unzip -oq "${arm64_zip}" -d "${arm64_dir}"

  x64_bin="${x64_dir}/cc-switch"
  arm64_bin="${arm64_dir}/cc-switch"
  [[ -f "${x64_bin}" ]] || die "x64 artifact did not contain cc-switch"
  [[ -f "${arm64_bin}" ]] || die "arm64 artifact did not contain cc-switch"

  info "Artifact checksums"
  sha256sum "${x64_bin}" "${arm64_bin}"

  case "$(uname -m)" in
    x86_64|amd64)
      install_local_binary "${x64_bin}"
      ;;
    aarch64|arm64)
      install_local_binary "${arm64_bin}"
      ;;
    *)
      die "Unsupported local architecture: $(uname -m)"
      ;;
  esac

  remote_install \
    "node13" \
    "${CCSWITCH_REMOTE_13_HOST}" \
    "${CCSWITCH_REMOTE_13_PORT}" \
    "${CCSWITCH_REMOTE_13_USER}" \
    "${CCSWITCH_REMOTE_13_PASSWORD}" \
    "${CCSWITCH_REMOTE_13_SUDO_PASSWORD}" \
    "${CCSWITCH_REMOTE_13_TARGET}" \
    "${x64_bin}"

  remote_install \
    "ugreen" \
    "${CCSWITCH_UGREEN_HOST}" \
    "${CCSWITCH_UGREEN_PORT}" \
    "${CCSWITCH_UGREEN_USER}" \
    "${CCSWITCH_UGREEN_PASSWORD}" \
    "${CCSWITCH_UGREEN_SUDO_PASSWORD}" \
    "${CCSWITCH_UGREEN_TARGET}" \
    "${x64_bin}"

  remote_install \
    "pantherx2" \
    "${CCSWITCH_PANTHERX2_HOST}" \
    "${CCSWITCH_PANTHERX2_PORT}" \
    "${CCSWITCH_PANTHERX2_USER}" \
    "${CCSWITCH_PANTHERX2_PASSWORD}" \
    "${CCSWITCH_PANTHERX2_SUDO_PASSWORD}" \
    "${CCSWITCH_PANTHERX2_TARGET}" \
    "${arm64_bin}"

  info "Done"
}

main "$@"
