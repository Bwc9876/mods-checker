# Outer Wilds Mods Checker

This action is meant to be used in the [Outer Wilds Mods Database](https://github.com/ow-mods/ow-mod-db) for checking the validity of newly added mods.

It can also be used in mod repositories to check if your build is valid.

## What It Checks

- The GitHub repository exists
- The repository has a release with a ZIP file as an asset
- The ZIP file contains a `manifest.json` file
- The `manifest.json` file is valid JSON
- The `manifest.json` file has the same unique name as the one in the issue form*
- The `manifest.json` file has the same version as the tag of the release
- The `manifest.json` file is pointing to a DLL file
- The unique name of the mod is not already in the database**

*: Only is `expectedUniqueName` is set  
**: Only if `skipDuplicateCheck` is not set

## Inputs

### `sourceType`

The type of source to fetch the mod from. This can be one of the following

- `repo`
- `url`
- `file`

### `source`

The source to fetch the mod from, depending on the `sourceType`:

- `repo`: The repository to fetch the mod from, in the format `owner/repo`
- `url`: The URL to fetch the mod from
- `file`: The path to the mod's ZIP file

### `expectedUniqueName`

What unique name the mod is expected to have. This is used in the database to make sure the unique name of the mod in the issue form matches the one in the manifest.

### `skipDuplicateCheck`

Normally the checker will check if the mod already exists in the database. If you want to skip this check, set this to `true`.

## Outputs

### `resultsJson`

The results of the check in JSON format.

```json
{
    "warnings": [],
    "error": null
}
```

See the enums defined in `main.rs` for the possible warnings and errors. There can only be one error, but there can be multiple warnings.
