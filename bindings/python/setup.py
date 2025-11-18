"""
Setup script for GraphLite Python bindings
"""

from setuptools import setup, find_packages
from pathlib import Path

# Read README
readme_file = Path(__file__).parent / "README.md"
long_description = readme_file.read_text() if readme_file.exists() else ""

setup(
    name="graphlite",
    version="0.1.0",
    author="DeepGraph Inc.",
    author_email="info@deepgraph.ai",
    description="Python bindings for GraphLite embedded graph database",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/deepgraph/graphlite",
    packages=find_packages(),
    classifiers=[
        "Development Status :: 3 - Alpha",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: Apache Software License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
        "Operating System :: OS Independent",
        "Topic :: Database",
        "Topic :: Software Development :: Libraries :: Python Modules",
    ],
    python_requires=">=3.8",
    install_requires=[
        # No dependencies - uses ctypes from standard library
    ],
    extras_require={
        "dev": [
            "pytest>=7.0.0",
            "pytest-cov>=4.0.0",
            "black>=23.0.0",
            "mypy>=1.0.0",
        ],
    },
    keywords="graph database gql embedded graphlite",
    project_urls={
        "Bug Reports": "https://github.com/deepgraph/graphlite/issues",
        "Source": "https://github.com/deepgraph/graphlite",
        "Documentation": "https://github.com/deepgraph/graphlite/tree/main/bindings/python",
    },
)
