# Dev guide

## Install 
- [Nix](https://nixos.org/download)
- Docker
- direnv

      nix profile install nixpkgs#direnv

- direnv [extension](https://marketplace.visualstudio.com/items?itemName=mkhl.direnv) for VSCode
- kubectl

#### Setup kubernetes cluster

#### Finalize
    direnv allow
    alias k=kubectl

## Backend 
> from /backend directory

#### Dev start

    cargo run

#### Env vars
    BE__ENV  [type: String] [default: local] { local prod }
    BE__PORT [type: u16]    [default: 8000]
    BE__HOST [type: String] [default: 127.0.0.1]

#### Run tests
    
    cargo test


## Frontend 
> from /frontend directory

#### Dev start

    trunk serve


## Frontend server 
> from /fe_server directory

#### Dev start

    cargo run

#### Env vars

    FE_SRV__ENV                   [type: String]       [default: local] { local prod }
    FE_SRV__PORT                  [type: u16]          [default: 9000]
    FE_SRV__HOST                  [type: String]       [default: 127.0.0.1]
    FE_SRV__DIR                   [type: Dir]          !
    FE_SRV__FALLBACK              [type: Option<Dir>]  [default: None]
    FE_SRV__REQUEST_PATH_LRU_SIZE [type: NonZeroUsize] [default: 30]

#### Run tests

    cargo test

## Docker image building on linux_x86_64 systems

    nix build .#BEdockerImage
    nix build .#FEdockerImage

## k8s cluster
#### Apply changes

    sh /k8s/k-apply-all.sh

#### Observe
    k get deploy be fe
    k get svc be fe
    k get ing snaking

#### Route external traffic to Ingress Controller
[Guide](https://www.digitalocean.com/community/developer-center/how-to-install-and-configure-ingress-controller-using-nginx)