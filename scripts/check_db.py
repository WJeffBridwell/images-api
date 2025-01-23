from pymongo import MongoClient
from bson.json_util import dumps
import json

# Connect to MongoDB
client = MongoClient('mongodb://localhost:27017/')
db = client.media_metadata

# Get collection stats
stats = db.command("collstats", "content")
doc_count = db.content.count_documents({})

print(f"\nMongoDB Collection Stats:")
print(f"Document count: {doc_count:,}")
print(f"Size: {stats['size']/1024/1024:.2f} MB")
print(f"Storage size: {stats['storageSize']/1024/1024:.2f} MB")
print(f"Average document size: {stats['avgObjSize']/1024:.2f} KB")
