-----

# VixenLens

**VixenLens** is a high-performance metadata indexing and retrieval engine specifically engineered for the VRChat ecosystem. Originally based on VRCXPhotoSearcher, this version has been overhauled with custom logic to provide a faster, more robust way to traverse and manage massive VRChat snapshot libraries.

[日本語版READMEはこちら](https://www.google.com/search?q=./README.ja.md)

-----

### Overview

VixenLens works as a high-context bridge between your in-game experiences and your local archives. By analyzing the metadata injected into PNGs by **[VRCX](https://github.com/vrcx-team/VRCX/)**, VixenLens allows for 4D-chess-level organization and searchability of your historical data.

<div style="display: flex; gap: 10px;">
<div>
<img src="./screenshots/topscreenshot.png" alt="VixenLens Interface" width="200">
<p>Main Interface</p>
</div>
<div>
<img src="./screenshots/search.png" alt="Search Functionality" width="200">
<p>Advanced Search</p>
</div>
<div>
<img src="./screenshots/settings.png" alt="Configuration" width="200">
<p>Settings</p>
</div>
</div>

### Why VixenLens?

This project has been updated with a focus on **scalability and architectural integrity**. It is designed for power users and developers who need a reliable, lightweight tool that doesn't buckle under the weight of thousands of files.

  * **Optimized Metadata Parsing:** Custom logic built to index usernames and world data with higher precision.
  * **Scalable Performance:** Developed with **Rust and Tauri** for a near-zero footprint and rapid query speeds even in massive directories.
  * **Deep Traversal:** Search by specific dates, world instances, or specific users found within the metadata.
  * **Zero-Cloud Privacy:** All processing remains 100% local. Your data stays on your machine, period.
  * **Developer-Ready:** Clean code architecture that follows professional standards, making it easy to maintain and extend.

### Getting Started

#### 1\. Requirements

VixenLens requires **VRCX** to be configured to save metadata into your screenshots. Ensure VRCX is active when you take photos in-game.

#### 2\. Initial Indexing

On your first launch, point VixenLens to your VRChat photo directory (usually `Pictures/VRChat`).

> **Pro Tip:** If you have years of archives, use the **Folder Exclusion** settings to narrow the scan range for maximum performance.

#### 3\. Advanced Search

Combine filters to find exactly what you need:

  * **Temporal Search:** Filter by Year/Month/Day.
  * **Spatial Search:** Filter by World Name.
  * **Identity Search:** Find snapshots featuring specific usernames.

-----

### Installation

#### Developer Build (Recommended)

1.  **Clone the Engine:**
    ```bash
    git clone https://github.com/Vixenlicious/VixenLens.git
    cd VixenLens
    ```
2.  **Install Dependencies:**
    Ensure you have **Rust**, **NodeJS**, and **Git Bash** installed.
    ```bash
    npm install
    ```
3.  **Build:**
    ```bash
    npm run tauri build
    ```
    Your optimized executable will be located in `src-tauri/target/release`.

-----

### Credits

VixenLens is maintained by **Vixenlicious**.

  * **YouTube:** [Vixenlicious](https://www.google.com/search?q=https://www.youtube.com/%40vixenlicious)
  * **GitHub:** [Vixenlicious](https://www.google.com/search?q=https://github.com/Vixenlicious)

*Special thanks to the original VRCXPhotoSearcher project for the initial inspiration.*

-----

### A Note on the Rebrand

As an IT professional with experience in critical infrastructure, I’ve rebuilt the backend of this tool to ensure it handles data more efficiently. **VixenLens** represents a shift toward a more professional, scalable toolset for the VRChat community.
