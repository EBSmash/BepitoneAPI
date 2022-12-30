{
  description = "Bepitone";

  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  inputs.deploy-rs.url = "github:serokell/deploy-rs";

  outputs = { self, nixpkgs, deploy-rs }:
  let
    pkgs = import nixpkgs { system = "x86_64-linux"; };

    bepitone_api = with pkgs; rustPlatform.buildRustPackage {
      pname = "bepitone_api";
      version = "1.0.0";
      src = ./.;
      cargoBuildFlags = [ "--workspace" ];

      cargoHash = "sha256-8c24euqoyljUnr8/bfVHCLCWCufdNXUXSdYwN8+Qtu0=";
    };
  in {
    nixosConfigurations.bepitone = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ({modulesPath, ...}: {
            imports = [ "${modulesPath}/virtualisation/amazon-image.nix" ];

            networking.firewall.allowedTCPPorts = [ 80 ];
            systemd.services.bepitone = {
              description = "bep";
              wantedBy = [ "multi-user.target" ];
              restartIfChanged = true;

              serviceConfig = {
                ExecStart = "${bepitone_api}/bin/bepitone_api";
                Restart = "on-failure";
              };
            };
            systemd.services.bepitone_discord = {
              description = "bep discord bot";
              wantedBy = [ "multi-user.target" ];
              restartIfChanged = true;

              serviceConfig = {
                ExecStart = "${bepitone_api}/bin/bepitone_discord";
                Restart = "on-failure";
              };
            };

            environment.systemPackages = with pkgs; [
              sqlite-interactive
              cloud-utils
            ];
        })
      ];
      inherit pkgs;
    };

    deploy.nodes.bepitone-ec2 = {
      hostname = "bep";

      profiles.main = {
        sshUser = "root";
        user = "root";
        path = deploy-rs.lib.x86_64-linux.activate.nixos self.nixosConfigurations.bepitone;
      };
    };

    packages.x86_64-linux.bepitone_api = bepitone_api;
  };
}
