{
  description = "Unseen Servant (usv) — a security-first server for the small networks";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "unseen-servant";
          version = "1.0.0";
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            # Only what the binary crate actually needs — fuzz/ has its
            # own separate Cargo.lock and would otherwise drag an
            # unrelated workspace into the build's source hash.
            filter = path: type:
              let base = baseNameOf path; in
              base != "fuzz" && base != "target" && base != ".git";
          };
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "--bin" "usv" ];
          doCheck = true;
          # --lib/--bins only, not the tests/ integration binary. Found
          # live: tests/smoke.rs spawns real usv subprocesses that bind a
          # listener and wait for it to come up (zero_arg_usv_serves_and_
          # drains_on_sigterm, sighup_reload_reaches_the_watcher_...) —
          # under Nix's build sandbox those hang indefinitely rather than
          # failing fast, and two other smoke tests that only check
          # subprocess exit code/stderr text (nonexistent_explicit_config_
          # is_a_startup_error, a_writable_titan_zone_without_fingerprints_
          # refuses_to_start) failed outright, reproducibly, in a genuinely
          # clean single-attempt build (an initial run's failures were
          # first — wrongly — blamed on a stale concurrent build process;
          # killing that and retrying alone reproduced the exact same
          # failures, ruling resource contention out). All consistent with
          # the sandbox's restricted network namespace, not a code bug:
          # the same tests pass in `nix develop`'s interactive shell and
          # in every other packaging format's real-environment test (CI,
          # the .deb/AUR/RPM containers). Nothing is actually skipped from
          # overall coverage — pr-checks.yml already runs the full `cargo
          # test` including tests/smoke.rs on a real, unsandboxed runner;
          # this only narrows what the *Nix build itself* re-checks.
          cargoTestFlags = [ "--lib" "--bins" ];
          meta = with pkgs.lib; {
            description = "A security-first Gemini capsule server that publishes one content tree to Geminispace and the web";
            homepage = "https://forgejo.wanderingmonster.dev/WanderingMonster/unseen-servant";
            license = licenses.mit;
            mainProgram = "usv";
            platforms = platforms.unix;
          };
        };

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/usv";
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [ cargo rustc rustfmt clippy cargo-fuzz ];
        };
      }
    );
}
