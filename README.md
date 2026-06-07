# Gerrit Circus Bridge

Gerrit changes -> [Circus CI](https://github.com/manic-system/circus) ->
Verified label.

Polls Gerrit for open changes with unreviewed patchsets, triggers Circus
evaluations, and posts Verified labels back.

> [!WARNING]
> This probably won't work for you until Circus has proper support for private
> repos. Right now I'm using a
> [fork](https://github.com/plumj-am/circus/tree/patch/PlumJam-rykzunzvwxxu)
> with private HTTP repo support bolted on.

## Overview

TODO

## Prerequisites

You will need a "Verified" label with the following configuration:

```config
[label "Verified"]
  TODO
```

You will also need a "circus" user with the following permissions:

- Label "Verified"
- Read access for the necessary repository

> [!IMPORTANT]
> Make sure the circus user has a HTTP password set!

## Usage

### NixOS

Add to your flake inputs:

```nix
{
  inputs.gerrit-circus-bridge = {
    url = "git+https://git.plumj.am/plumjam/gerrit-circus-bridge";
    inputs.nixpkgs.follows = "nixpkgs";
  };
}
```

Import the module and use it:

```nix
{
  imports = [ inputs.gerrit-circus-bridge.nixosModules.default ];

  services.gerrit-circus-bridge = {
    enable = true;
    circusApiKeyFile = config.age.secrets.circusGerritApiKey.path;
    gerritPasswordFile = config.age.secrets.circusGerritPassword.path;
  };
}
```

Available options:

| Key    | Default | Description |
| ------ | ------- | ----------- |
| `todo` | `todo`  | todo        |

The secrets file must contain the password in this format:

TODO: check

```env
GERRIT_HTTP_PASSWORD=xxxxxxx
```

For a complete usage example see my [Circus module]().

## Contributing

I'll gladly accept contributions. Please open a PR or an issue on GitHub.

## License

```
The MIT License (MIT)

Copyright (c) 2026-present PlumJam <git@plumj.am>

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
