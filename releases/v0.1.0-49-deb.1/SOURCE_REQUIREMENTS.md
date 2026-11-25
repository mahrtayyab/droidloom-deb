# Public binary-release checklist

**Pending review. Staging a local candidate is not public publication.**
Do not mark this candidate ready merely because its checksum and notices pass.

The retained Droidloom notice requires complete corresponding source for
GPL-covered binary distributions. A patch repository, a GPL text, or a link to
one top-level source commit alone is not proof that all obligations for the
bundled runtime are covered.

Before taking a draft release public:

- [ ] Record the final compatibility-repository commit/tag used for the release.
- [ ] Review the exact `.deb` and the three inputs in `packaging/inputs-49.json`.
- [ ] Provide complete corresponding source/build/install inputs for covered
      binaries, or verify equivalent-access source hosting and put clear,
      durable links beside the binary download as required by the applicable
      licenses. Do not use a promise of future source as a substitute.
- [ ] Include Droidloom's pinned upstream source and packaging modifications.
- [ ] Follow the upstream source locks for Android/AOSP, Mesa, the native bridge,
      Rust dependencies, and any other covered components; determine which
      source archives/notices must accompany this exact binary.
- [ ] Check the retained Android notice catalog, Mesa material, Cargo license
      inventory, and other component notices against the shipped payload.
- [ ] Confirm no Google/proprietary add-ons or personal app/data payloads were
      introduced. Any separately licensed redistributable material needs its own
      review; the project's GPL grant does not relicense it.
- [ ] Record target-distribution tests and known limitations without claiming
      a general Debian port or integrated fixes.
- [ ] Put the resolved source locations and review results in the release notes
      and provenance, then update the candidate status deliberately.

Starting points, not a claim of completed source compliance:

- Upstream source:
  <https://github.com/denialwm/droidloom/tree/572e1c51c5caf75183efe25ad811f98c1268bf56>
- Upstream input locks:
  <https://github.com/denialwm/droidloom/tree/572e1c51c5caf75183efe25ad811f98c1268bf56/android/manifest>
- Upstream license scope:
  <https://github.com/denialwm/droidloom/blob/572e1c51c5caf75183efe25ad811f98c1268bf56/THIRD_PARTY.md>

The current candidate contains a historical binary repack, not a newly compiled
runtime with a freshly assembled complete-source release bundle. Resolve the
remaining review before publication.
