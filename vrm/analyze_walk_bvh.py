"""
BVH walk animation frame data analysis.
Check the actual rotation values for lThigh across all frames.
"""

def parse_bvh(filename):
    with open(filename, 'r') as f:
        content = f.read()
    
    lines = content.split('\n')
    
    # Find bone structure
    bones = []
    bone_stack = []
    current_bone = None
    i = 0
    
    while i < len(lines):
        line = lines[i].strip()
        
        if line.startswith('ROOT') or line.startswith('JOINT'):
            parts = line.split()
            name = parts[1]
            current_bone = {'name': name, 'channels': [], 'parent': bone_stack[-1] if bone_stack else None}
            bones.append(current_bone)
            bone_stack.append(current_bone)
        elif line.startswith('CHANNELS'):
            parts = line.split()
            n = int(parts[1])
            channels = parts[2:2+n]
            current_bone['channels'] = channels
        elif line == '{':
            pass
        elif line == '}':
            if bone_stack:
                bone_stack.pop()
                current_bone = bone_stack[-1] if bone_stack else None
        elif line.startswith('MOTION'):
            break
        i += 1
    
    # Parse motion data
    frames_section = False
    frames_data = []
    n_frames = 0
    
    for line in lines[i:]:
        line = line.strip()
        if line.startswith('Frames:'):
            n_frames = int(line.split(':')[1].strip())
        elif line.startswith('Frame Time:'):
            frames_section = True
        elif frames_section and line:
            values = [float(v) for v in line.split()]
            frames_data.append(values)
    
    return bones, frames_data

def get_bone_frame_data(bones, frames_data, bone_name, frame_idx):
    """Get rotation data for a bone at a specific frame."""
    # Calculate channel offset for this bone
    offset = 0
    for bone in bones:
        if bone['name'] == bone_name:
            if not bone['channels']:
                return None
            values = frames_data[frame_idx][offset:offset+len(bone['channels'])]
            return dict(zip(bone['channels'], values))
        offset += len(bone['channels'])
    return None

bones, frames_data = parse_bvh('/Users/logue/Developer/vrm2sl/frontend/public/animations/avatar_walk.bvh')

print(f"Total bones: {len(bones)}")
print(f"Total frames: {len(frames_data)}")

# Print all channels for each frame for hip and lThigh
print("\n--- hip (mPelvis) per frame ---")
for fi in range(len(frames_data)):
    data = get_bone_frame_data(bones, frames_data, 'hip', fi)
    print(f"  Frame {fi:2d}: {data}")

print("\n--- lThigh (mHipLeft) per frame ---")
for fi in range(len(frames_data)):
    data = get_bone_frame_data(bones, frames_data, 'lThigh', fi)
    print(f"  Frame {fi:2d}: {data}")

print("\n--- rThigh (mHipRight) per frame ---")
for fi in range(len(frames_data)):
    data = get_bone_frame_data(bones, frames_data, 'rThigh', fi)
    print(f"  Frame {fi:2d}: {data}")

print("\n--- lShin (mKneeLeft) per frame ---")
for fi in range(len(frames_data)):
    data = get_bone_frame_data(bones, frames_data, 'lShin', fi)
    print(f"  Frame {fi:2d}: {data}")
