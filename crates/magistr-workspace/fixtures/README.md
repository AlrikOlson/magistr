# Magistr workspace fixtures

`plugins/` contains four rebuilt Rust guest components, their source, current manifests, artifact hashes and third-party notices. `examples/plugins/` contains the echo and language-pack SDK examples adapted to the current `magistr` WIT namespace and publisher.

All six originate in the pinned workspace reference import recorded in `subsystems/workspace/PROVENANCE.md` and are owned test inputs. The reference copy stays unchanged for provenance. Native tests use these current copies. See `plugins/README.md` for Rust guest rebuild instructions and license-evidence limitations; normal Cargo builds require no reference checkout or guest compiler.
