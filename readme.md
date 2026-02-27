# Road to 2.0

## What needs to change?

1. **The Entity setup**

   Currently the entity handling (WGPU glue, bindings, buffers, meshes) are too tightly coupled together. It is exceedingly harder to adjust and/or add alternative setups.
   To boil it down, if we look at the three different aspects of graphical rendering. We have
   - Geometry: Vertex buffers and layout. The concrete vertex data.
   - Materials: Pipeline and bindgroups, essentially how stuff is drawn. What texture, what settings.
   - Instances: Take the above and replicate with transformations

   Right now all of the above is mashed into one RenderController that hardcodes bindgroups across different aspects. Geometry vertex buffers are in the same MeshController as the instances, Pipelines are hardcoded for specific vertex types etc etc

   I want to split this up into the above three approaches
