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
	features: CpuFeatures;
	cache_l1_kb?: number;
	cache_l2_kb?: number;
	cache_l3_kb?: number;
}

export interface CpuFeatures {
	// x86/x86_64 features
	sse42: boolean;
	avx: boolean;
	avx2: boolean;
	avx512f: boolean;
	fma: boolean;
	// ARM features
	neon: boolean;
	sve: boolean;
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
