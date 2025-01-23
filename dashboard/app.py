import streamlit as st
import pandas as pd
import plotly.express as px
from pathlib import Path
from pymongo import MongoClient

# Initialize MongoDB connection
client = MongoClient('mongodb://localhost:27017/')
db = client['media']
models_collection = db['models']
content_collection = db['content']

# Content directories from load_all_content.sh
CONTENT_DIRECTORIES = [
    "/Users/jeffbridwell/VideosAa-Abella",
    "/Volumes/VideosAbella-Alexa",
    "/Volumes/VideosAlexa-Ame",
    "/Volumes/VideosAme-Aria",
    "/Volumes/VideosAria-Bianca",
    "/Volumes/VideosBianka-Chan",
    "/Volumes/VideosChan-Coco",
    "/Volumes/VideosCoco-Eliza",
    "/Volumes/VideosEliza-Erica",
    "/Volumes/VideosErica-Haley",
    "/Volumes/VideosHaley-Hime",
    "/Volumes/VideosHime-Jeff",
    "/Volumes/VideosJeff-Kata",
    "/Volumes/VideosNew/New/VideosKata-Kenn",
    "/Volumes/PhotosNew/VideosKenn-Kenz",
    "/Users/jeffbridwell/VideosKenzie-Kev",
    "/Volumes/VideosKey-Lea",
    "/Volumes/VideosLeb-Luci",
    "/Volumes/VideosLucj-Maria",
    "/Volumes/VideosMaria-Mega",
    "/Volumes/VideosMega-Mia",
    "/Volumes/VideosMia-Nat",
    "/Volumes/VideosNew/VideosNat-Nia",
    "/Volumes/VideosNia-Rilex",
    "/Users/jeffbridwell/VideosRiley",
    "/Volumes/VideosRilez-Ta",
    "/Volumes/VideosTb-Uma",
    "/Volumes/VideosUma-Zaa",
    "/Volumes/VideosNew/VideosZaa-Zz"
]

def get_models_data(search_term=None, tag_filter=None):
    # Base pipeline
    pipeline = []
    
    # Match stage for filters
    match_conditions = {}
    if search_term:
        match_conditions['filename'] = {'$regex': search_term, '$options': 'i'}
    if tag_filter:
        match_conditions['macos_attributes.mdls.kMDItemUserTags'] = tag_filter
    if match_conditions:
        pipeline.append({'$match': match_conditions})
    
    # Group by path first to deduplicate
    pipeline.append({
        '$group': {
            '_id': '$path',
            'path': {'$first': '$path'},
            'filename': {'$first': '$filename'},
            'size': {'$first': '$base_attributes.size'}
        }
    })

    # Add directory field
    pipeline.append({
        '$addFields': {
            'directory': {
                '$let': {
                    'vars': {
                        'parts': {'$split': ['$path', '/']}
                    },
                    'in': {
                        '$reduce': {
                            'input': {'$range': [0, {'$size': '$$parts'}]},
                            'initialValue': '',
                            'in': {
                                '$cond': {
                                    'if': {'$eq': ['$$this', {'$subtract': [{'$size': '$$parts'}, 2]}]},
                                    'then': {
                                        '$arrayElemAt': ['$$parts', '$$this']
                                    },
                                    'else': '$$value'
                                }
                            }
                        }
                    }
                }
            }
        }
    })
    
    # Group by directory
    pipeline.append({
        '$group': {
            '_id': '$directory',
            'count': {'$sum': 1},
            'total_size': {'$sum': '$size'},
            'files': {'$push': {
                'filename': '$filename',
                'size': '$size'
            }}
        }
    })
    
    results = list(models_collection.aggregate(pipeline))
    
    # Convert to DataFrame
    df = pd.DataFrame(results)
    if not df.empty:
        df['total_size_gb'] = df['total_size'] / (1024 * 1024 * 1024)  # Convert to GB
        df['avg_size_gb'] = df.apply(lambda x: x['total_size'] / (len(x['files']) * 1024 * 1024 * 1024), axis=1)
        df = df.rename(columns={'_id': 'directory'})
    else:
        df = pd.DataFrame(columns=['directory', 'count', 'total_size', 'total_size_gb', 'avg_size_gb'])
    
    return df

def get_available_tags():
    return models_collection.distinct('macos_attributes.mdls.kMDItemUserTags')

def get_content_data(file_type=None, directory=None, search_term=None):
    # Base pipeline
    pipeline = []
    
    # Match stage for filters
    match_conditions = {}
    
    if file_type:
        if file_type == 'vr':
            match_conditions['file_path'] = {'$regex': '-vr-', '$options': 'i'}
            match_conditions['content_type'] = {'$regex': '^video/', '$options': 'i'}
        elif file_type == 'image':
            match_conditions['content_type'] = {'$regex': '^image/', '$options': 'i'}
        elif file_type == 'video':
            match_conditions['content_type'] = {'$regex': '^video/', '$options': 'i'}
        elif file_type == 'archive':
            match_conditions['content_type'] = {'$regex': 'zip|rar|7z', '$options': 'i'}
    
    if directory:
        match_conditions['file_path'] = {'$regex': f'^{directory}', '$options': 'i'}
    
    if search_term:
        match_conditions['file_path'] = {'$regex': search_term, '$options': 'i'}
    
    if match_conditions:
        pipeline.append({'$match': match_conditions})

    # Group by file_path first to deduplicate
    pipeline.append({
        '$group': {
            '_id': '$file_path',
            'file_path': {'$first': '$file_path'},
            'content_type': {'$first': '$content_type'},
            'size': {'$first': '$base_attributes.size'}
        }
    })
    
    # Add directory field
    pipeline.append({
        '$addFields': {
            'directory': {
                '$let': {
                    'vars': {
                        'parts': {'$split': ['$file_path', '/']}
                    },
                    'in': {
                        '$reduce': {
                            'input': {'$range': [0, {'$size': '$$parts'}]},
                            'initialValue': '',
                            'in': {
                                '$cond': {
                                    'if': {'$eq': ['$$this', {'$subtract': [{'$size': '$$parts'}, 2]}]},
                                    'then': {
                                        '$arrayElemAt': ['$$parts', '$$this']
                                    },
                                    'else': '$$value'
                                }
                            }
                        }
                    }
                }
            }
        }
    })
    
    # Then group by directory and type
    pipeline.append({
        '$group': {
            '_id': {
                'directory': '$directory',
                'type': {
                    '$cond': {
                        'if': {'$regexMatch': {'input': '$content_type', 'regex': '^image/', 'options': 'i'}},
                        'then': 'image',
                        'else': {
                            '$cond': {
                                'if': {'$regexMatch': {'input': '$content_type', 'regex': '^video/', 'options': 'i'}},
                                'then': 'video',
                                'else': {
                                    '$cond': {
                                        'if': {'$regexMatch': {'input': '$content_type', 'regex': 'zip|rar|7z', 'options': 'i'}},
                                        'then': 'archive',
                                        'else': 'other'
                                    }
                                }
                            }
                        }
                    }
                }
            },
            'count': {'$sum': 1},
            'total_size': {'$sum': '$size'}
        }
    })
    
    results = list(content_collection.aggregate(pipeline, allowDiskUse=True))
    
    if not results:
        return pd.DataFrame(columns=['directory', 'type', 'count', 'total_size', 'total_size_tb', 'avg_size_tb'])
    
    # Convert to DataFrame
    df = pd.DataFrame(results)
    df['directory'] = df['_id'].apply(lambda x: x.get('directory', 'Unknown'))
    df['type'] = df['_id'].apply(lambda x: x.get('type', 'Unknown'))
    df['total_size_tb'] = df['total_size'] / (1024 * 1024 * 1024 * 1024)  # Convert to TB
    df['avg_size_tb'] = df['total_size'] / (df['count'] * 1024 * 1024 * 1024 * 1024)  # Convert to TB
    df = df.drop('_id', axis=1)
    
    return df

# Set page config
st.set_page_config(page_title="Media Analytics Dashboard", layout="wide")

# Title
st.title("Media Analytics Dashboard")

# Create tabs
tab1, tab2 = st.tabs(["Models Collection", "Content Collection"])

with tab1:
    # Add filters
    col1, col2 = st.columns(2)
    
    with col1:
        search_term = st.text_input("Search by filename", key="models_search")
    
    with col2:
        tag_filter = st.selectbox("Filter by tag", ["All"] + get_available_tags(), key="models_tag")
        if tag_filter == "All":
            tag_filter = None

    # Load data
    df_models = get_models_data(search_term, tag_filter)
    
    # Display metrics
    if not df_models.empty:
        col1, col2, col3 = st.columns(3)
        
        total_files = df_models['count'].sum()
        total_size_gb = df_models['total_size_gb'].sum()  # Use the GB column directly
        
        with col1:
            st.metric("Total Files", f"{total_files:,}")
        with col2:
            st.metric("Total Size (GB)", f"{total_size_gb:.2f}")  # Already in GB
        with col3:
            st.metric("Average Size (GB)", f"{(total_size_gb / total_files):.2f}")  # Already in GB

        # Distribution by directory
        st.subheader("Files Distribution by Directory")
        fig1 = px.bar(df_models, 
                     x="directory", y="count",
                     title="File Count by Directory")
        st.plotly_chart(fig1)

        # Size distribution
        st.subheader("Size Distribution by Directory")
        fig2 = px.bar(df_models, 
                     x="directory", y="total_size_gb",
                     title="Total Size (GB) by Directory")
        st.plotly_chart(fig2)

        # Data table
        st.subheader("Detailed Statistics")
        st.dataframe(df_models[[
            'directory', 'count', 'total_size_gb', 'avg_size_gb'
        ]].sort_values('count', ascending=False))
    else:
        st.warning("No data found matching the current filters.")

with tab2:
    # Filters
    col1, col2, col3 = st.columns(3)
    
    with col1:
        file_type = st.selectbox(
            "Filter by type",
            ["All", "image", "video", "vr", "archive"],
            key="content_type"
        )
        if file_type == "All":
            file_type = None
    
    with col2:
        directory = st.selectbox(
            "Filter by directory",
            ["All"] + CONTENT_DIRECTORIES,
            key="content_directory"
        )
        if directory == "All":
            directory = None
    
    with col3:
        search_term = st.text_input("Search by filename", key="content_search")

    # Load data
    df_content = get_content_data(file_type, directory, search_term)
    
    # Display metrics
    if not df_content.empty:
        col1, col2, col3 = st.columns(3)
        
        total_files = df_content['count'].sum()
        total_size_tb = df_content['total_size_tb'].sum()  # Use the TB column directly
        
        with col1:
            st.metric("Total Files", f"{total_files:,}")
        with col2:
            st.metric("Total Size (TB)", f"{total_size_tb:.2f}")  # Already in TB
        with col3:
            st.metric("Average Size (TB)", f"{(total_size_tb / total_files):.2f}")  # Already in TB

        # Distribution by type
        st.subheader("Files Distribution by Type")
        df_by_type = df_content.groupby('type').agg({
            'count': 'sum',
            'total_size_tb': 'sum'
        }).reset_index()
        
        col1, col2 = st.columns(2)
        with col1:
            fig1 = px.pie(df_by_type, 
                         values="count", names="type",
                         title="File Count by Type")
            st.plotly_chart(fig1)
        
        with col2:
            fig2 = px.pie(df_by_type, 
                         values="total_size_tb", names="type",
                         title="Total Size by Type (TB)")
            st.plotly_chart(fig2)

        # Distribution by directory
        st.subheader("Files Distribution by Directory")
        df_by_dir = df_content.groupby('directory').agg({
            'count': 'sum',
            'total_size_tb': 'sum'
        }).reset_index()
        
        fig3 = px.bar(df_by_dir, 
                     x="directory", y="count",
                     title="File Count by Directory")
        st.plotly_chart(fig3)

        fig4 = px.bar(df_by_dir, 
                     x="directory", y="total_size_tb",
                     title="Total Size (TB) by Directory")
        st.plotly_chart(fig4)

        # Data table
        st.subheader("Detailed Statistics")
        st.dataframe(df_content[[
            'directory', 'type', 'count', 'total_size_tb', 'avg_size_tb'
        ]].sort_values(['directory', 'type']))
    else:
        st.warning("No data found matching the current filters.")
