use std::{
    collections::{BTreeMap, btree_map::Entry},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::core::buffer::{Buffer, BufferKey};

static NEXT_TRANSIENT_RESOURCE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub(crate) enum MaterialResourceKey {
    Buffer(BufferKey),
    TextureView { texture: u64, index: u32 },
    Sampler { texture: u64 },
    TransientTextureView(u64),
    TransientSampler(u64),
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub(crate) struct MaterialBindingKey {
    pub group: u32,
    pub binding: u32,
    pub resource: MaterialResourceKey,
    pub layout: wgpu::BindGroupLayoutEntry,
}

#[derive(Clone)]
pub(crate) enum MaterialBinding {
    Buffer {
        buffer: Buffer,
    },
    TextureView {
        view: wgpu::TextureView,
        view_dimension: wgpu::TextureViewDimension,
        sample_type: wgpu::TextureSampleType,
        visibility: wgpu::ShaderStages,
        resource_key: MaterialResourceKey,
    },
    Sampler {
        sampler: wgpu::Sampler,
        binding_type: wgpu::SamplerBindingType,
        visibility: wgpu::ShaderStages,
        resource_key: MaterialResourceKey,
    },
}

impl MaterialBinding {
    fn layout_entry(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
        match self {
            Self::Buffer { buffer } => buffer.layout_entry(binding),
            Self::TextureView {
                view_dimension,
                sample_type,
                visibility,
                ..
            } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: *visibility,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: *view_dimension,
                    sample_type: *sample_type,
                },
                count: None,
            },
            Self::Sampler {
                binding_type,
                visibility,
                ..
            } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: *visibility,
                ty: wgpu::BindingType::Sampler(*binding_type),
                count: None,
            },
        }
    }

    fn bind_group_entry(&self, binding: u32) -> wgpu::BindGroupEntry<'_> {
        match self {
            Self::Buffer { buffer } => buffer.bind_group_entry(binding),
            Self::TextureView { view, .. } => wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(view),
            },
            Self::Sampler { sampler, .. } => wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        }
    }

    fn resource_key(&self) -> MaterialResourceKey {
        match self {
            Self::Buffer { buffer } => MaterialResourceKey::Buffer(buffer.key.clone()),
            Self::TextureView { resource_key, .. } | Self::Sampler { resource_key, .. } => {
                resource_key.clone()
            }
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct BindGroupBuilder {
    bindings: BTreeMap<u32, BTreeMap<u32, MaterialBinding>>,
}

impl BindGroupBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn buffer(&mut self, buffer: &Buffer, group: u32, binding: u32) -> &mut Self {
        self.insert(
            group,
            binding,
            MaterialBinding::Buffer {
                buffer: buffer.clone(),
            },
        );
        self
    }

    pub(crate) fn contains(&self, group: u32, binding: u32) -> bool {
        self.bindings
            .get(&group)
            .is_some_and(|bindings| bindings.contains_key(&binding))
    }

    pub(crate) fn contains_buffer(&self, buffer: &Buffer) -> bool {
        self.bindings.values().any(|bindings| {
            bindings.values().any(|binding| {
                matches!(binding, MaterialBinding::Buffer { buffer: registered } if registered.key == buffer.key)
            })
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn texture_view(
        &mut self,
        view: &wgpu::TextureView,
        texture_id: u64,
        texture_index: u32,
        view_dimension: wgpu::TextureViewDimension,
        sample_type: wgpu::TextureSampleType,
        visibility: wgpu::ShaderStages,
        group: u32,
        binding: u32,
    ) -> &mut Self {
        self.insert(
            group,
            binding,
            MaterialBinding::TextureView {
                view: view.clone(),
                view_dimension,
                sample_type,
                visibility,
                resource_key: MaterialResourceKey::TextureView {
                    texture: texture_id,
                    index: texture_index,
                },
            },
        );
        self
    }

    pub(crate) fn transient_texture_view(
        &mut self,
        view: &wgpu::TextureView,
        view_dimension: wgpu::TextureViewDimension,
        sample_type: wgpu::TextureSampleType,
        visibility: wgpu::ShaderStages,
        group: u32,
        binding: u32,
    ) -> &mut Self {
        self.insert(
            group,
            binding,
            MaterialBinding::TextureView {
                view: view.clone(),
                view_dimension,
                sample_type,
                visibility,
                resource_key: MaterialResourceKey::TransientTextureView(
                    NEXT_TRANSIENT_RESOURCE_ID.fetch_add(1, Ordering::Relaxed),
                ),
            },
        );
        self
    }

    pub(crate) fn sampler(
        &mut self,
        sampler: &wgpu::Sampler,
        texture_id: u64,
        binding_type: wgpu::SamplerBindingType,
        visibility: wgpu::ShaderStages,
        group: u32,
        binding: u32,
    ) -> &mut Self {
        self.insert(
            group,
            binding,
            MaterialBinding::Sampler {
                sampler: sampler.clone(),
                binding_type,
                visibility,
                resource_key: MaterialResourceKey::Sampler {
                    texture: texture_id,
                },
            },
        );
        self
    }

    pub(crate) fn transient_sampler(
        &mut self,
        sampler: &wgpu::Sampler,
        binding_type: wgpu::SamplerBindingType,
        visibility: wgpu::ShaderStages,
        group: u32,
        binding: u32,
    ) -> &mut Self {
        self.insert(
            group,
            binding,
            MaterialBinding::Sampler {
                sampler: sampler.clone(),
                binding_type,
                visibility,
                resource_key: MaterialResourceKey::TransientSampler(
                    NEXT_TRANSIENT_RESOURCE_ID.fetch_add(1, Ordering::Relaxed),
                ),
            },
        );
        self
    }

    pub(crate) fn replace_buffer(&mut self, buffer: &Buffer, group: u32, binding: u32) {
        self.replace(
            group,
            binding,
            MaterialBinding::Buffer {
                buffer: buffer.clone(),
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn replace_texture_view(
        &mut self,
        view: &wgpu::TextureView,
        texture_id: u64,
        texture_index: u32,
        view_dimension: wgpu::TextureViewDimension,
        sample_type: wgpu::TextureSampleType,
        visibility: wgpu::ShaderStages,
        group: u32,
        binding: u32,
    ) {
        self.replace(
            group,
            binding,
            MaterialBinding::TextureView {
                view: view.clone(),
                view_dimension,
                sample_type,
                visibility,
                resource_key: MaterialResourceKey::TextureView {
                    texture: texture_id,
                    index: texture_index,
                },
            },
        );
    }

    pub(crate) fn replace_sampler(
        &mut self,
        sampler: &wgpu::Sampler,
        texture_id: u64,
        binding_type: wgpu::SamplerBindingType,
        visibility: wgpu::ShaderStages,
        group: u32,
        binding: u32,
    ) {
        self.replace(
            group,
            binding,
            MaterialBinding::Sampler {
                sampler: sampler.clone(),
                binding_type,
                visibility,
                resource_key: MaterialResourceKey::Sampler {
                    texture: texture_id,
                },
            },
        );
    }

    fn insert(&mut self, group: u32, binding: u32, resource: MaterialBinding) {
        match self.bindings.entry(group).or_default().entry(binding) {
            Entry::Vacant(entry) => {
                entry.insert(resource);
            }
            Entry::Occupied(_) => {
                panic!(
                    "duplicate material binding registration at group {group}, binding {binding}"
                )
            }
        }
    }

    fn replace(&mut self, group: u32, binding: u32, resource: MaterialBinding) {
        let existing = self
            .bindings
            .get_mut(&group)
            .and_then(|group_bindings| group_bindings.get_mut(&binding))
            .unwrap_or_else(|| {
                panic!(
                    "cannot replace missing material binding at group {group}, binding {binding}"
                )
            });
        *existing = resource;
    }

    pub(crate) fn keys(&self) -> Vec<MaterialBindingKey> {
        self.bindings
            .iter()
            .flat_map(|(&group, bindings)| {
                bindings
                    .iter()
                    .map(move |(&binding, resource)| MaterialBindingKey {
                        group,
                        binding,
                        resource: resource.resource_key(),
                        layout: resource.layout_entry(binding),
                    })
            })
            .collect()
    }

    pub(crate) fn build(&self, device: &wgpu::Device, label: &str) -> BuiltBindGroups {
        let group_count = self
            .bindings
            .last_key_value()
            .map(|(&group, _)| group as usize + 1)
            .unwrap_or(0);
        let mut layouts = vec![None; group_count];
        let mut bind_groups = vec![None; group_count];
        let mut layout_entries = vec![None; group_count];

        for (&group, bindings) in &self.bindings {
            let entries = bindings
                .iter()
                .map(|(&binding, resource)| resource.layout_entry(binding))
                .collect::<Vec<_>>();
            let group_label = format!("{label} group {group}");
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(&group_label),
                entries: &entries,
            });
            let bind_group = self.create_group(device, group, &layout, &group_label);
            layouts[group as usize] = Some(layout);
            bind_groups[group as usize] = Some(bind_group);
            layout_entries[group as usize] = Some(entries);
        }

        BuiltBindGroups {
            layouts,
            bind_groups,
            layout_entries,
        }
    }

    pub(crate) fn build_with_layouts(
        &self,
        device: &wgpu::Device,
        layouts: &[Option<wgpu::BindGroupLayout>],
        expected_entries: &[Option<Vec<wgpu::BindGroupLayoutEntry>>],
        label: &str,
    ) -> Vec<Option<wgpu::BindGroup>> {
        let group_count = layouts.len();
        let mut bind_groups = vec![None; group_count];
        for (&group, bindings) in &self.bindings {
            let layout = layouts
                .get(group as usize)
                .and_then(Option::as_ref)
                .unwrap_or_else(|| panic!("missing bind group layout for group {group}"));
            let actual_entries = bindings
                .iter()
                .map(|(&binding, resource)| resource.layout_entry(binding))
                .collect::<Vec<_>>();
            let expected = expected_entries
                .get(group as usize)
                .and_then(Option::as_ref)
                .unwrap_or_else(|| panic!("missing binding topology for group {group}"));
            assert_eq!(
                &actual_entries, expected,
                "replacement binding topology changed for group {group}"
            );
            bind_groups[group as usize] = Some(self.create_group(device, group, layout, label));
        }
        bind_groups
    }

    fn create_group(
        &self,
        device: &wgpu::Device,
        group: u32,
        layout: &wgpu::BindGroupLayout,
        label: &str,
    ) -> wgpu::BindGroup {
        let bindings = self
            .bindings
            .get(&group)
            .unwrap_or_else(|| panic!("missing resources for bind group {group}"));
        let entries = bindings
            .iter()
            .map(|(&binding, resource)| resource.bind_group_entry(binding))
            .collect::<Vec<_>>();
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout,
            entries: &entries,
        })
    }
}

#[derive(Clone)]
pub(crate) struct BuiltBindGroups {
    pub layouts: Vec<Option<wgpu::BindGroupLayout>>,
    pub bind_groups: Vec<Option<wgpu::BindGroup>>,
    pub layout_entries: Vec<Option<Vec<wgpu::BindGroupLayoutEntry>>>,
}
