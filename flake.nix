{
  nixConfig = {
    extra-substituters = [ "https://valeratrades.cachix.org" ];
    extra-trusted-public-keys = [ "valeratrades.cachix.org-1:gXVwhzO5YB+BaiEJYT48qZgzdaErGQew6xtZcz4Fo1Q=" ];
  };

  inputs = {
    v_flakes.url = "github:valeratrades/v_flakes?ref=v1.6";
  };

  outputs = { self, v_flakes }:
    let
      inherit (v_flakes) flake-utils pre-commit-hooks;
      manifest = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package;
      # The binary (crates/panel_server, `[[bin]] name`), the image and the release all go
      # by this name; the workspace root has no package of its own.
      pname = "panel";
    in
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import v_flakes.default_nixpkgs { inherit system; };
        lib = pkgs.lib;
        rust = v_flakes.rs.default_nightly system;
        # mold is Linux-only — the adapter throws at *eval* time on Darwin, which
        # would break every target on a mac, not just the ones that link.
        stdenv = if pkgs.stdenv.isDarwin then pkgs.stdenv else pkgs.stdenvAdapters.useMoldLinker pkgs.stdenv;

        # Single source for the container's exposed port and the prod bind. The binary's
        # own default (`panel_server::DEFAULT_BIND`) is the same port on 127.0.0.1.
        port = 59120;

        pre-commit-check = pre-commit-hooks.lib.${system}.run (v_flakes.files.preCommit { inherit pkgs; stripClaudeSignature = true; });
        rs = v_flakes.rs {
          inherit pkgs rust;
          build.workspace = {
            "./crates/panel_server" = [ "git_version" "log_directives" ];
          };
        };
        github = v_flakes.github {
          inherit pkgs pname rs;
          enable = true;
          lastSupportedVersion = "nightly-2026-09-03";
          containerRelease = { registry = "ghcr.io/service-arb"; };
          jobs.default = true;
          lfs = false;
        };
        readme = v_flakes.readme-fw {
          inherit pkgs pname;
          defaults = true;
          lastSupportedVersion = "nightly-1.100";
          rootDir = ./.;
          badges = [ "msrv" "loc" "ci" ];
        };
        combined = v_flakes.utils.combine { inherit rust; modules = [ rs github readme ]; };

        build_rust = v_flakes.rs.build_nightly system;
        rustPlatform = pkgs.makeRustPlatform { rustc = build_rust; cargo = build_rust; inherit stdenv; };
        # `.cargo` holds dev-only accelerators (sccache rustc-wrapper, cranelift,
        # mold) the hermetic sandbox lacks — drop it so the pure build uses nix's
        # own toolchain instead of failing on a missing `sccache` on PATH.
        pureSrc = lib.cleanSourceWith {
          src = lib.cleanSource ./.;
          filter = path: _type: baseNameOf path != ".cargo";
        };

        bin = rustPlatform.buildRustPackage {
          inherit pname;
          version = manifest.version;
          src = pureSrc;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "panel_server" ];
          nativeBuildInputs = with pkgs; [ pkg-config ];
          # ev_lib's `sentry` turns on reqwest's native-tls, which is OpenSSL on Linux
          # (Security.framework on Darwin, which needs nothing here).
          buildInputs = lib.optionals pkgs.stdenv.isLinux [ pkgs.openssl ];
          # the database tests need a Postgres the sandbox does not have; `cargo test`
          # runs them in the devShell and CI
          doCheck = false;
          auditable = false; # cargo-auditable doesn't support edition 2024
        };

        # The front end's static export (`next build` → out/), served by `panel serve` from
        # PANEL_WEB_DIR on the same origin as /api and /auth.
        frontend = pkgs.buildNpmPackage {
          pname = "${pname}-frontend";
          version = manifest.version;
          src = lib.cleanSourceWith {
            src = lib.cleanSource ./frontend;
            filter = path: _type: !(builtins.elem (baseNameOf path) [ "node_modules" ".next" "out" ]);
          };
          npmDepsHash = "sha256-M/PThVT69iMpeCigW+R1msLLR8PWXag3mcFV2ivYqlU=";
          env = {
            NEXT_TELEMETRY_DISABLED = "1";
          };
          # Turbopack talks to its node workers over a loopback socket, which the Darwin
          # sandbox refuses unless asked.
          __darwinAllowLocalNetworking = true;
          # Turbopack's build stalls in the sandbox after "Compiled successfully" (its
          # PostCSS worker never hands back); webpack makes the same export.
          npmBuildFlags = [ "--" "--webpack" ];
          # next writes its cache under HOME, which the sandbox does not have
          preBuild = ''
            export HOME="$TMPDIR"
          '';
          installPhase = ''
            runHook preInstall
            cp -r out "$out"
            runHook postInstall
          '';
        };

        # What the deploy must provide, beyond what `container.implement` records. Plain
        # data on the contract, read by the devops generator; `checks.contract-env` holds
        # `requiredEnv` to the binary's own `--print-required-vars`.
        runtimeRole = "panel_app";
        deploy = {
          # Settings::required_var_names("production"): the pod exits 78 without any of them
          requiredEnv = [ "DATABASE_URL" "PANEL_DATA_KEY" "PANEL_PUBLIC_ORIGIN" "CONCIERGE_PUBLIC_ORIGIN" "CONCIERGE_GRPC_ADDR" "RP_CLIENT_SECRET_SA" ];
          # from the sops-backed Secret, never literal env
          secretEnv = [ "DATABASE_URL" "MIGRATE_DATABASE_URL" "PANEL_DATA_KEY" "SENTRY_DSN" "RP_CLIENT_SECRET_SA" "TELEGRAM_BOT_TOKEN" "POSTHOG_PERSONAL_API_KEY" ];
          # without POSTHOG_PROJECT_ID and POSTHOG_PERSONAL_API_KEY the hourly import is off
          # (serve warns); one without the other fails the boot
          optionalEnv = [ "SENTRY_DSN" "TELEGRAM_BOT_TOKEN" "TELEGRAM_BOT_USERNAME" "TELEGRAM_LOCALE" "POSTHOG_API_HOST" "POSTHOG_PROJECT_ID" "POSTHOG_PERSONAL_API_KEY" ];
          postgres = {
            # the app's pods: the runtime role, holding deploy/panel_app.sql's grants only
            runtime = { env = "DATABASE_URL"; role = runtimeRole; };
            # the schema's owner: the migrate step alone carries it
            owner = { env = "MIGRATE_DATABASE_URL"; };
          };
          # Before the new pods start (an initContainer, or a Job the rollout waits on), in
          # this image: applies the build's migrations, then the runtime role's grants. It
          # reads the same settings as `serve` (APP_ENV=production requires `requiredEnv`),
          # plus MIGRATE_DATABASE_URL, which the app's own env must not carry.
          #
          # The binary by its image path, never its store path: Flux moves the image tag on
          # its own, and every release has a new store path, so a pinned one would name a file
          # the next image does not have (`contents` below puts it at /bin).
          migrate = {
            command = [ "/bin/${pname}" "migrate" "--grant-to" runtimeRole ];
            env = [ "MIGRATE_DATABASE_URL" ];
          };
          ingress = {
            # in-cluster only, by service DNS (docs/ARCHITECTURE.md, Deploy requirements)
            excludePathPrefixes = [ "/api/ingest" ];
            # per client IP at the edge; the panel bounds concurrency, not who calls
            rateLimitPathPrefixes = [ "/auth" ];
          };
          egress = {
            postgres = true;
            # concierge's gRPC, at the address CONCIERGE_GRPC_ADDR names
            grpcEnv = [ "CONCIERGE_GRPC_ADDR" ];
            # the Bot API; PostHog's query API (POSTHOG_API_HOST's default — follow it if it
            # is pointed elsewhere; the capture host us.i.posthog.com is not this one)
            hosts = [ "api.telegram.org:443" "us.posthog.com:443" ];
          };
        };

        containerStd = v_flakes.container.implement {
          inherit pkgs pname;
          containers."" = {
            inherit port;
            healthPath = "/health";
            # a source that gets a 5xx retries from its outbox; nothing is lost while down
            criticality = "normal";
            entrypoint = [ "${bin}/bin/${pname}" "serve" "--bind" "0.0.0.0:${toString port}" ];
            # /bin/panel, the path `deploy.migrate.command` names
            contents = [ bin ];
            # the production guards (`#[required_in("production")]`) are armed wherever it runs
            env = { APP_ENV = "production"; };
            imageEnv = [ "PANEL_WEB_DIR=${frontend}" ];
          };
        };
        # `implement` knows no key for the above, and throws on one it does not know.
        containers = lib.mapAttrs (_: c: c // { contract = c.contract // deploy; }) containerStd.containers;

        contractEnv = pkgs.runCommand "${pname}-contract-env" { } ''
          ${bin}/bin/${pname} --print-required-vars production | sort > got
          printf '%s\n' ${lib.escapeShellArgs deploy.requiredEnv} | sort > want
          diff -u want got || { echo "flake.nix deploy.requiredEnv disagrees with the binary" >&2; exit 1; }
          touch "$out"
        '';

        help = pkgs.writeShellApplication {
          name = "help";
          text = ''
            cat <<'EOF'
            nix develop                       toolchain, postgresql
            nix build                         the panel binary
            nix build .#${pname}-container    OCI image, the front end in it (Linux only)
            nix build .#frontend              the front end's static export
            nix run .#help                    this
            cargo test                        unit tests; the database ones need DATABASE_URL
                                              (a server the tests may CREATE DATABASE on)
            the CLI itself: panel --help
            EOF
          '';
        };
      in
      {
        apps = {
          help = { type = "app"; program = lib.getExe help; };
        };

        packages = {
          default = bin;
          inherit bin frontend;
        } // lib.optionalAttrs pkgs.stdenv.isLinux containerStd.packages;

        containers = lib.optionalAttrs pkgs.stdenv.isLinux containers;

        checks.contract-env = contractEnv;

        devShells.default =
          with pkgs;
          mkShell {
            inherit stdenv;
            shellHook =
              pre-commit-check.shellHook
              + combined.shellHook
              + ''
                cp -f ${(v_flakes.files.treefmt) { inherit pkgs; }} ./.treefmt.toml
                cp -f ${(v_flakes.files.gitattributes) { inherit pkgs; lfs = false; }} ./.gitattributes
              '';

            packages = [
              mold
              pkg-config
              openssl # sentry's native-tls
              postgresql # psql, and a local server for the database tests
              rust
            ] ++ pre-commit-check.enabledPackages ++ combined.enabledPackages;

            env = {
              RUST_BACKTRACE = 1;
              RUST_LIB_BACKTRACE = 0;
            };
          };
      }
    );
}
