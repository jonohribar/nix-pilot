# Phase 0 metadata exporter.
#
# Given a Nixpkgs package set (`pkgs`) and a list of top-level attribute names,
# return one record per name. Every field is forced to a JSON-safe primitive
# (null | string | list of strings | bool) under tryEval, and each entry is
# wrapped in its own tryEval so a single broken or pseudo package yields an
# evalError record instead of sinking the whole export. This is a study
# instrument: it measures evaluation cost and field completeness without
# building anything.
#
# Usage (impure flake import):
#   nix eval --impure --json --expr \
#     '(import ./export.nix) { pkgs = (builtins.getFlake "nixpkgs").legacyPackages.x86_64-linux; names = [ "hello" ]; }'
{
  pkgs,
  names,
}: let
  # Force any scalar to a string; null if absent, a compound value, or
  # not coercible to a string (e.g. a function).
  scalar = v:
    if v == null then null
    else if builtins.isAttrs v || builtins.isList v then null
    else let r = builtins.tryEval (builtins.toString v);
    in if r.success then r.value else null;

  one = name: let
    d = pkgs.${name};
    isDrv = builtins.isAttrs d && (d ? type) && d.type == "derivation";
    pname = if isDrv then scalar (d.pname or null) else null;
    version = if isDrv then scalar (d.version or null) else null;
    m =
      if isDrv then builtins.tryEval (d.meta or {})
      else {success = false; value = null;};
    meta = if m.success && builtins.isAttrs m.value then m.value else null;
    desc = if meta == null then null else scalar (meta.description or null);
    hp = if meta == null then null else scalar (meta.homepage or null);
    mp = if meta == null then null else scalar (meta.mainProgram or null);
    licOk = meta != null && (meta ? license) && (builtins.tryEval meta.license).success;
    licType =
      if meta == null || !(meta ? license) then null
      else let r = builtins.tryEval meta.license;
      in if r.success then builtins.typeOf r.value else "unreadable";
    # Real license-freedom signal (decision D1/D17): whether any license in the
    # package's license set reports free == false. Tri-state: true = all present
    # are free, false = at least one is known-unfree, null = indeterminate or no
    # licensing information. Used by the optional hide-unfree filter (l3q.2).
    licFree =
      if meta == null || !(meta ? license) then null
      else let r = builtins.tryEval meta.license;
      in
      if !r.success then null
      else let
        one =
          x:
            if !builtins.isAttrs x then null
            else let rr = builtins.tryEval (x.free or null);
            in if rr.success && builtins.isBool rr.value then rr.value else null;
        l = if builtins.isList r.value then r.value else [r.value];
        flags = map one (builtins.filter builtins.isAttrs l);
      in
      if builtins.length flags == 0 then null
      # Indeterminate: a licence without a free field reports no freedom signal.
      else if builtins.any (x: x == null) flags then null
      else if builtins.any (x: x == false) flags then false
      else true;
    plats =
      if meta == null || !(meta ? platforms) then []
      else let
        r = builtins.tryEval meta.platforms;
        one = e: if builtins.typeOf e == "string" then e else "<nonstring>";
      in if builtins.isList r.value then map one r.value
      else if builtins.typeOf r.value == "string" then [r.value]
      else [];
    brk =
      if meta == null || !(meta ? broken) then null
      else let r = builtins.tryEval meta.broken;
      in if r.success && builtins.isBool r.value then r.value else null;
  in {
    name = name;
    isDerivation = isDrv;
    pname = pname;
    version = version;
    description = desc;
    homePage = hp;
    mainProgram = mp;
    licenseOk = licOk;
    licenseType = licType;
    licenseFree = licFree;
    platforms = plats;
    broken = brk;
  };
  # Each entry is deep-forced inside tryEval so every field is a JSON-safe
  # primitive by the time the encoder sees it; a failing entry yields an
  # evalError record instead of sinking the run.
  guarded = name: let
    v = one name;
    r = builtins.tryEval (builtins.deepSeq v v);
  in if r.success then v else {name = name; isDerivation = false; evalError = true;};
in map guarded names