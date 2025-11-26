export interface HardwareInfo {
	cpu: CpuInfo;
	gpus: GpuInfo[];
	npu?: NpuInfo;
	memory: MemoryInfo;
	storage: StorageInfo;
	detected_at: string;
}

export interface CpuInfo {
	vendor: string;
	brand: string;
	architecture: string;
	cores_physical: number;
	cores_logical: number;
	frequency_mhz?: number;
	features: string[]; // CPUFeature enum variants from hardware-query
	cache_l1_kb?: number;
	cache_l2_kb?: number;
	cache_l3_kb?: number;
}

// CPU feature types from hardware-query CPUFeature enum
export type CpuFeature =
	| 'AVX'
	| 'AVX2'
	| 'AVX512'
	| 'SSE'
	| 'SSE2'
	| 'SSE3'
	| 'SSE41'
	| 'SSE42'
	| 'FMA'
	| 'AES'
	| 'SHA'
	| 'BMI1'
	| 'BMI2'
	| 'RDRAND'
	| 'RDSEED'
	| 'POPCNT'
	| 'LZCNT'
	| 'MOVBE'
	| 'PREFETCHWT1'
	| 'CLFLUSHOPT'
	| 'CLWB'
	| 'XSAVE'
	| 'XSAVEOPT'
	| 'XSAVEC'
	| 'XSAVES'
	| 'FSGSBASE'
	| 'RDTSCP'
	| 'F16C';

// Helper function to check if a CPU has a specific feature
export function hasCpuFeature(cpu: CpuInfo, feature: CpuFeature): boolean {
	return cpu.features.includes(feature);
}

export interface GpuInfo {
	name: string;
	vendor: 'Nvidia' | 'Amd' | 'Intel' | 'Apple' | 'Qualcomm' | 'Unknown';
	backend: string;
	device_type: string;
	vram_mb?: number;
	temperature_c?: number;
	utilization_percent?: number;
	cuda_compute_capability?: string;
}

export interface NpuInfo {
	detected: boolean;
	confidence: 'High' | 'Medium' | 'Low';
	details: string;
	method: string;
}

export interface MemoryInfo {
	total_gb: number;
	available_gb: number;
}

export interface StorageInfo {
	total_gb: number;
	available_gb: number;
	is_ssd: boolean;
	device_name: string;
}
