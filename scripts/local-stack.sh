#!/usr/bin/env bash
# The panel and both landings on this machine, wired together: a lead posted on a site lands
# in the panel, a place edited in the panel shows on the site. See docs/LOCAL.md.
#
#   nix run .#local-stack                  build, seed, start everything; Ctrl-C stops it all
#   nix run .#local-stack -- --reset       wipe .local/ (database, keys, the sites' leads) first
#   nix run .#local-stack -- --no-sites    the panel alone
#   nix run .#local-stack -- --role operator
#
# The flake app sets PANEL_BIN and PANEL_WEB_DIR to its own builds; run directly, this
# builds them with `nix build`, or takes yours (PANEL_BIN=target/debug/panel for a cargo
# build). AQUAFIX_DIR and VIFNET_DIR name the sites' checkouts, by default beside the panel's.
set -euo pipefail

PANEL_PORT=59120
AQUAFIX_PORT=59081
VIFNET_PORT=59082
AQUAFIX_PLACES=(royat clermont-ferrand desgenettes lyon-est la-mouche lyon-nord)
VIFNET_PLACES=(vifnet)

reset=0
sites=1
role="admin"
while [ $# -gt 0 ]; do
	case "$1" in
	--reset) reset=1 ;;
	--no-sites) sites=0 ;;
	--role)
		role="${2:-}"
		shift
		;;
	-h | --help)
		cat <<'EOF'
local-stack [--reset] [--no-sites] [--role admin|operator]

  The panel on http://127.0.0.1:59120 (dev sign-in), aquafix on :59081 and vifnet on :59082,
  wired to it. State in .local/ (--reset wipes it). Ctrl-C stops everything. See docs/LOCAL.md.
EOF
		exit 0
		;;
	*)
		echo "unknown argument: $1 (see --help)" >&2
		exit 2
		;;
	esac
	shift
done
case "$role" in
admin | operator) ;;
*)
	echo "--role is admin or operator, not '$role'" >&2
	exit 2
	;;
esac

say() { printf '\033[1m▶ %s\033[0m\n' "$*" >&2; }
die() {
	printf '\033[31m✗ %s\033[0m\n' "$*" >&2
	exit 1
}

root="$(git rev-parse --show-toplevel 2>/dev/null)" || die "run this from inside the panel checkout"
[ -f "$root/crates/panel_server/Cargo.toml" ] || die "$root is not the panel checkout"
# The sites sit beside the panel's main checkout (service_arb/{panel,aquafix,vifnet}), also
# when this is one of its worktrees.
main_checkout="$(dirname "$(git -C "$root" rev-parse --path-format=absolute --git-common-dir)")"
service_arb="$(dirname "$main_checkout")"
AQUAFIX_DIR="${AQUAFIX_DIR:-$service_arb/aquafix}"
VIFNET_DIR="${VIFNET_DIR:-$service_arb/vifnet}"

state="${LOCAL_STACK_DIR:-$root/.local}"
if [ "$reset" = 1 ] && [ -d "$state" ]; then
	say "--reset: wiping $state"
	rm -rf "$state"
fi
mkdir -p "$state/logs" "$state/secrets"
# Ignored by its own .gitignore: the repo's is generated, and this is nobody else's business.
printf '*\n' >"$state/.gitignore"

# ── ports ────────────────────────────────────────────────────────────────────────────────

busy() { (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null; }
ports=("$PANEL_PORT")
[ "$sites" = 1 ] && ports+=("$AQUAFIX_PORT" "$VIFNET_PORT")
for port in "${ports[@]}"; do
	busy "$port" && die "port $port is taken — another local stack, or \`lsof -nP -iTCP:$port -sTCP:LISTEN\` says who"
done

# ── the panel's builds ───────────────────────────────────────────────────────────────────

if [ -z "${PANEL_BIN:-}" ]; then
	say "building the panel (nix build .#bin)"
	nix build "$root#bin" --out-link "$state/build/bin"
	PANEL_BIN="$state/build/bin/bin/panel"
fi
if [ -z "${PANEL_WEB_DIR:-}" ]; then
	say "building the front end (nix build .#frontend)"
	nix build "$root#frontend" --out-link "$state/build/frontend"
	PANEL_WEB_DIR="$state/build/frontend"
fi
[ -x "$PANEL_BIN" ] || die "PANEL_BIN $PANEL_BIN is not an executable"

# ── the panel's environment ──────────────────────────────────────────────────────────────

[ -s "$state/data-key" ] || "$PANEL_BIN" gen-data-key >"$state/data-key"
origin="http://127.0.0.1:$PANEL_PORT"
# Whatever the calling shell exports for a deployed panel must not leak in: concierge's
# variables would refuse the dev sign-in, PostHog's would start the hourly import.
panel() {
	env -u CONCIERGE_PUBLIC_ORIGIN -u CONCIERGE_GRPC_ADDR -u RP_CLIENT_SECRET_SA \
		-u TELEGRAM_BOT_TOKEN -u POSTHOG_PROJECT_ID -u POSTHOG_PERSONAL_API_KEY -u SENTRY_DSN \
		APP_ENV=development \
		PANEL_DB_PATH="$state/panel.db" \
		PANEL_DATA_KEY="$(cat "$state/data-key")" \
		PANEL_PUBLIC_ORIGIN="$origin" \
		PANEL_DEV_SIGN_IN="$role" \
		PANEL_WEB_DIR="$PANEL_WEB_DIR" \
		"$PANEL_BIN" "$@"
}

# ── seed: idempotent, so every run may do it ─────────────────────────────────────────────

say "seeding $state/panel.db"
panel migrate
# A source's secret is shown once: kept in .local/secrets/<key id> for the next run.
source_secret() {
	local key_id="$1" brand="$2" file="$state/secrets/$1"
	if panel source list | awk -v k="$key_id" '$1 == k && $NF == "active" { found = 1 } END { exit !found }'; then
		[ -s "$file" ] || die "source $key_id exists but its secret is not in $file — start over with --reset"
	else
		panel source add "$key_id" --kind site --brand "$brand" | awk '$1 == "secret:" { print $2 }' >"$file"
		[ -s "$file" ] || die "panel source add $key_id printed no secret"
	fi
	cat "$file"
}
AQUAFIX_SECRET="$(source_secret aquafix-site aquafix)"
VIFNET_SECRET="$(source_secret vifnet-site vifnet)"
register() {
	panel place register "$1" "$2" >/dev/null 2>>"$state/logs/seed.log" || die "registering $1/$2 failed: see $state/logs/seed.log"
}
for slug in "${AQUAFIX_PLACES[@]}"; do register aquafix "$slug"; done
for slug in "${VIFNET_PLACES[@]}"; do register vifnet "$slug"; done

# ── run ──────────────────────────────────────────────────────────────────────────────────

# Job control: each background job below gets a process group of its own, which a signal
# stops whole — npm and next under `nix run .#dev` included — without touching whoever started
# this script (`kill 0` would, when that is a non-interactive shell sharing our group).
set -m
groups=()
stop_all() {
	trap - INT TERM EXIT
	echo >&2
	say "stopping"
	local pg
	for pg in ${groups[@]+"${groups[@]}"}; do kill -TERM -- "-$pg" 2>/dev/null || true; done
	for _ in $(seq 10); do
		local alive=0
		for pg in ${groups[@]+"${groups[@]}"}; do kill -0 -- "-$pg" 2>/dev/null && alive=1; done
		[ "$alive" = 0 ] && return 0
		sleep 1
	done
	for pg in ${groups[@]+"${groups[@]}"}; do kill -KILL -- "-$pg" 2>/dev/null || true; done
}
trap stop_all EXIT
trap 'stop_all; exit 130' INT TERM

# Each line prefixed with whose it is, and kept in .local/logs/<name>.log.
prefixed() {
	local name="$1"
	shift
	"$@" 2>&1 | tee -a "$state/logs/$name.log" | awk -v p="[$name] " '{ print p $0; fflush() }'
}

# Polls `url` while `pid` lives, at most `tries` seconds.
wait_for() {
	local name="$1" url="$2" tries="$3" pid="$4"
	for _ in $(seq "$tries"); do
		curl -fsS -o /dev/null --max-time 5 "$url" 2>/dev/null && return 0
		kill -0 "$pid" 2>/dev/null || {
			echo "$name stopped before answering $url — see $state/logs/$name.log" >&2
			return 1
		}
		sleep 1
	done
	echo "$name did not answer $url in ${tries}s — see $state/logs/$name.log" >&2
	return 1
}

say "starting the panel on $origin (dev sign-in: $role)"
prefixed panel panel serve --bind "127.0.0.1:$PANEL_PORT" &
panel_pid=$!
groups+=("$panel_pid")
wait_for panel "$origin/health" 30 "$panel_pid" || die "the panel did not start"

started=()
# A site's own dev command (`nix run .#dev`, kitstart's next dev) in its checkout, with what
# the panel needs of it; its leads in a file of ours, analytics off.
start_site() {
	local name="$1" dir="$2" port="$3" brand="$4" key_id="$5" secret="$6"
	if [ ! -f "$dir/package.json" ] || [ ! -f "$dir/flake.nix" ]; then
		echo "skipping $name: $dir is not a kitstart checkout (missing, or older than the Next site — git pull)" >&2
		return 0
	fi
	say "starting $name from $dir on :$port"
	(
		cd "$dir"
		prefixed "$name" env -u POSTHOG_KEY -u POSTHOG_HOST -u LEADS_DB_URL \
			LEAD_WEBHOOK_URL="$origin/api/ingest/v1/events" \
			LEAD_WEBHOOK_KEY_ID="$key_id" \
			LEAD_WEBHOOK_SECRET="$secret" \
			LOCATIONS_API_URL="$origin/api/internal/brands/$brand" \
			LEADS_DB_PATH="$state/$name-leads.db" \
			nix run .#dev
	) &
	groups+=("$!")
	started+=("$name:$port:$!")
}
if [ "$sites" = 1 ]; then
	start_site aquafix "$AQUAFIX_DIR" "$AQUAFIX_PORT" aquafix aquafix-site "$AQUAFIX_SECRET"
	start_site vifnet "$VIFNET_DIR" "$VIFNET_PORT" vifnet vifnet-site "$VIFNET_SECRET"
	for s in ${started[@]+"${started[@]}"}; do
		IFS=: read -r name port pid <<<"$s"
		# The first `nix run .#dev` may install node_modules: minutes, not seconds.
		wait_for "$name" "http://localhost:$port/health" 600 "$pid" || true
	done
fi

{
	echo
	echo "────────────────────────────────────────────────────────────────────────"
	echo " panel    $origin   (opens signed in: dev sign-in, $role)"
	echo "          leads $origin/leads/   places $origin/places/"
	for s in ${started[@]+"${started[@]}"}; do
		case "${s%%:*}" in
		aquafix)
			for slug in "${AQUAFIX_PLACES[@]}"; do echo " aquafix  http://$slug.localhost:$AQUAFIX_PORT/fr"; done
			;;
		vifnet) echo " vifnet   http://localhost:$VIFNET_PORT/fr" ;;
		esac
	done
	echo
	echo " state    $state   (--reset wipes it)   logs $state/logs/"
	echo " stop     Ctrl-C stops everything"
	echo "────────────────────────────────────────────────────────────────────────"
} >&2

# Until Ctrl-C, or until the panel stops (then everything does). Polled rather than `wait -n`,
# which macOS's bash 3.2 lacks.
while kill -0 "$panel_pid" 2>/dev/null; do sleep 1; done
say "the panel stopped — see $state/logs/panel.log"
